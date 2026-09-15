use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use crate::{
        models::{Pagination, PaginationParamsWithSearch},
        response::{ApiResponse, ApiResponseResult},
        routes::{ApiError, GetState, api::user::admin::NodeStatus},
    };
    use axum::http::StatusCode;
    use axum_extra::extract::Query;
    use indexmap::IndexMap;
    use serde::Serialize;
    use sqlx::Row;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[schema(rename_all = "camelCase")]
    struct Bandwidth {
        ip: Option<String>,
        organization_id: Option<i32>,
        organization_name: Option<compact_str::CompactString>,
        verified: bool,

        used: i64,
        limit: i64,
        reset: i64,

        #[schema(inline)]
        nodes: IndexMap<compact_str::CompactString, i64>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        success: bool,

        #[schema(inline)]
        nodes: Vec<NodeStatus>,
        #[schema(inline)]
        bandwidth: Pagination<Bandwidth>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = inline(ApiError)),
    ), params(
        (
            "page" = i64, Query,
            description = "The page number (starting from 1)",
            minimum = 1,
            example = 1,
        ),
        (
            "per_page" = i64, Query,
            description = "The number of items per page (maximum 200)",
            minimum = 1,
            maximum = 200,
            example = 50,
        ),
        (
            "search" = String, Query,
            description = "Filter entries by IP address or organization name",
        ),
    ))]
    pub async fn route(
        state: GetState,
        params: Query<PaginationParamsWithSearch>,
    ) -> ApiResponseResult {
        if let Err(errors) = crate::utils::validate_data(&params.0) {
            return ApiResponse::new_serialized(serde_json::json!({
                "success": false,
                "errors": errors,
            }))
            .with_status(StatusCode::BAD_REQUEST)
            .ok();
        }

        let local = crate::requests::bandwidth_snapshot(&state.cache).await?;
        let fanned = super::super::fanout_with_local(&state, "/bandwidth", local).await?;

        let mut merged: IndexMap<(Option<String>, Option<i32>), Bandwidth> = IndexMap::new();
        for (node, snapshots) in fanned.data {
            for snapshot in snapshots {
                let entry = merged
                    .entry((snapshot.ip.clone(), snapshot.organization_id))
                    .or_insert_with(|| Bandwidth {
                        ip: snapshot.ip,
                        organization_id: snapshot.organization_id,
                        organization_name: None,
                        verified: false,
                        used: 0,
                        limit: crate::requests::bandwidth_limit_for(false),
                        reset: snapshot.reset,
                        nodes: IndexMap::new(),
                    });

                entry.used += snapshot.used;
                entry.reset = entry.reset.min(snapshot.reset);
                entry.nodes.insert(node.clone(), snapshot.used);
            }
        }

        let organization_ids: Vec<i32> = merged
            .values()
            .filter_map(|entry| entry.organization_id)
            .collect();

        if !organization_ids.is_empty() {
            let rows = sqlx::query(
                r#"
                SELECT organizations.id, organizations.name, organizations.verified
                FROM organizations
                WHERE organizations.id = ANY($1)
                "#,
            )
            .bind(&organization_ids)
            .fetch_all(state.database.read())
            .await?;

            let organizations: IndexMap<i32, (compact_str::CompactString, bool)> = rows
                .iter()
                .map(|row| {
                    Ok::<_, anyhow::Error>((row.try_get(0)?, (row.try_get(1)?, row.try_get(2)?)))
                })
                .collect::<Result<_, _>>()?;

            for entry in merged.values_mut() {
                if let Some((name, verified)) =
                    entry.organization_id.and_then(|id| organizations.get(&id))
                {
                    entry.organization_name = Some(name.clone());
                    entry.verified = *verified;
                    entry.limit = crate::requests::bandwidth_limit_for(*verified);
                }
            }
        }

        let mut bandwidth: Vec<Bandwidth> = merged
            .into_values()
            .filter(|entry| {
                params.search.as_ref().is_none_or(|search| {
                    entry
                        .ip
                        .as_ref()
                        .is_some_and(|ip| ip.contains(search.as_str()))
                        || entry
                            .organization_name
                            .as_ref()
                            .is_some_and(|name| name.contains(search.as_str()))
                })
            })
            .collect();
        bandwidth.sort_by_key(|entry| std::cmp::Reverse(entry.used));

        let total = bandwidth.len() as i64;
        let offset = ((params.page - 1) * params.per_page).min(total) as usize;

        ApiResponse::new_serialized(Response {
            success: true,
            nodes: fanned.nodes,
            bandwidth: Pagination {
                total,
                per_page: params.per_page,
                page: params.page,
                data: bandwidth
                    .drain(offset..)
                    .take(params.per_page as usize)
                    .collect(),
            },
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
