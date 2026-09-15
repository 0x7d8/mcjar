use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use crate::{
        nodes::SystemSnapshot,
        response::{ApiResponse, ApiResponseResult},
        routes::GetState,
    };
    use serde::{Deserialize, Serialize};
    use sqlx::Row;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize, Deserialize)]
    struct StatsRequests {
        total: i64,

        minute: u64,
        hour: u64,
        day: u64,
        week: u64,
        month: u64,
        year: u64,
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    struct StatsNodes {
        total: i64,
        alive: i64,
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    struct Stats {
        organizations: i64,
        users: i64,
        sessions: i64,
        webhooks: i64,

        #[schema(inline)]
        nodes: StatsNodes,
        #[schema(inline)]
        requests: StatsRequests,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        success: bool,

        #[schema(inline)]
        stats: Stats,
        system: SystemSnapshot,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState) -> ApiResponseResult {
        let stats = state
            .cache
            .cached("stats::admin::all", 60, || async {
                let (data, requests_data) = tokio::try_join!(
                    async {
                        let data = sqlx::query(
                            r#"
                            SELECT COUNT(*)
                            FROM organizations
                            UNION ALL
                            SELECT COUNT(*)
                            FROM users
                            UNION ALL
                            SELECT COUNT(*)
                            FROM user_sessions
                            UNION ALL
                            SELECT COUNT(*)
                            FROM webhooks
                            UNION ALL
                            SELECT COUNT(*)
                            FROM nodes
                            UNION ALL
                            SELECT COUNT(*)
                            FROM nodes
                            WHERE nodes.last_seen > NOW() - INTERVAL '1 minute'
                            UNION ALL
                            SELECT COALESCE((SELECT value FROM counts WHERE key = 'requests'), 0)
                            "#,
                        )
                        .fetch_all(state.database.read())
                        .await?;

                        Ok::<_, anyhow::Error>(data)
                    },
                    async {
                        let requests_data = state.clickhouse
                            .client()
                            .query(
                                r#"
                                SELECT
                                    countIf(requests.created > NOW() - INTERVAL '1 minute'),
                                    countIf(requests.created > NOW() - INTERVAL '1 hour'),
                                    countIf(requests.created > NOW() - INTERVAL '1 day'),
                                    countIf(requests.created > NOW() - INTERVAL '1 week'),
                                    countIf(requests.created > NOW() - INTERVAL '1 month'),
                                    COUNT(*)
                                FROM requests
                                WHERE requests._partition_date >= toDate(now() - INTERVAL 366 DAY)
                                "#
                            )
                            .fetch_one::<(u64, u64, u64, u64, u64, u64)>()
                            .await?;

                        Ok(requests_data)
                    },
                )?;

                Ok::<_, anyhow::Error>(Stats {
                    organizations: data[0].try_get(0)?,
                    users: data[1].try_get(0)?,
                    sessions: data[2].try_get(0)?,
                    webhooks: data[3].try_get(0)?,
                    nodes: StatsNodes {
                        total: data[4].try_get(0)?,
                        alive: data[5].try_get(0)?,
                    },
                    requests: StatsRequests {
                        total: data[6].try_get(0)?,

                        minute: requests_data.0,
                        hour: requests_data.1,
                        day: requests_data.2,
                        week: requests_data.3,
                        month: requests_data.4,
                        year: requests_data.5,
                    },
                })
            })
            .await?;

        ApiResponse::new_serialized(Response {
            success: true,
            stats,
            system: SystemSnapshot::capture(&state).await?,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
