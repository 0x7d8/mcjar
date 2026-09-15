use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

pub const WINDOWS: [(&str, &str, u16); 4] = [
    ("hour", "1 MINUTE", 1),
    ("day", "30 MINUTE", 1),
    ("week", "6 HOUR", 7),
    ("month", "1 DAY", 31),
];

mod get {
    use crate::{
        response::{ApiResponse, ApiResponseResult},
        routes::{ApiError, GetState},
    };
    use axum::http::StatusCode;
    use axum_extra::extract::Query;
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;

    #[derive(Deserialize)]
    pub struct Params {
        #[serde(default = "default_window")]
        window: compact_str::CompactString,
    }

    fn default_window() -> compact_str::CompactString {
        compact_str::CompactString::const_new("day")
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[schema(rename_all = "camelCase")]
    struct TimelinePoint {
        time: i64,
        total: u64,
        unique_ips: u64,
        errors: u64,
        avg_time: f64,
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    struct StatusCount {
        status: i16,
        total: u64,
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[schema(rename_all = "camelCase")]
    struct SlowPath {
        path: String,
        total: u64,
        avg_time: f64,
        p95_time: f64,
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[schema(rename_all = "camelCase")]
    struct TopEntry {
        label: String,
        total: u64,
        unique_ips: u64,
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[schema(rename_all = "camelCase")]
    struct TopIp {
        ip: String,
        total: u64,
        errors: u64,
        user_agents: u64,
        country: Option<String>,
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[schema(rename_all = "camelCase")]
    struct Requests {
        window: compact_str::CompactString,

        total: u64,
        unique_ips: u64,
        errors: u64,
        avg_time: f64,
        p95_time: f64,

        #[schema(inline)]
        timeline: Vec<TimelinePoint>,
        #[schema(inline)]
        statuses: Vec<StatusCount>,
        #[schema(inline)]
        slowest: Vec<SlowPath>,
        #[schema(inline)]
        ips: Vec<TopIp>,
        #[schema(inline)]
        user_agents: Vec<TopEntry>,
        #[schema(inline)]
        origins: Vec<TopEntry>,
        #[schema(inline)]
        countries: Vec<TopEntry>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        success: bool,

        #[schema(inline)]
        requests: Requests,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = inline(ApiError)),
    ), params(
        (
            "window" = String, Query,
            description = "The time window to report on",
            example = "day",
        ),
    ))]
    pub async fn route(state: GetState, params: Query<Params>) -> ApiResponseResult {
        let Some((window, bucket, partition_days)) = super::WINDOWS
            .iter()
            .find(|(name, _, _)| *name == params.window.as_str())
            .copied()
        else {
            return ApiResponse::error("invalid window")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        };

        let requests = state
            .cache
            .cached(&format!("stats::admin::requests::{window}"), 60, || async {
                let since = format!("NOW() - INTERVAL 1 {}", window.to_uppercase());
                let partition = format!(
                    "_partition_date >= toDate(now() - INTERVAL {partition_days} DAY)"
                );

                let (totals, timeline, statuses, slowest, ips, user_agents, origins, countries) = tokio::try_join!(
                    async {
                        Ok::<_, anyhow::Error>(
                            state
                                .clickhouse
                                .client()
                                .query(&format!(
                                    r#"
                                    SELECT
                                        COUNT(*),
                                        uniqExact(ip),
                                        countIf(status >= 400),
                                        avg(time),
                                        quantile(0.95)(time)
                                    FROM requests
                                    WHERE {partition} AND created > {since}
                                    "#
                                ))
                                .fetch_one::<(u64, u64, u64, f64, f64)>()
                                .await?,
                        )
                    },
                    async {
                        Ok(state
                            .clickhouse
                            .client()
                            .query(&format!(
                                r#"
                                SELECT
                                    toUnixTimestamp(toStartOfInterval(created, INTERVAL {bucket})) AS time,
                                    COUNT(*),
                                    uniqExact(ip),
                                    countIf(status >= 400),
                                    avg(time)
                                FROM requests
                                WHERE {partition} AND created > {since}
                                GROUP BY time
                                ORDER BY time ASC
                                "#
                            ))
                            .fetch_all::<(u32, u64, u64, u64, f64)>()
                            .await?)
                    },
                    async {
                        Ok(state
                            .clickhouse
                            .client()
                            .query(&format!(
                                r#"
                                SELECT status, COUNT(*)
                                FROM requests
                                WHERE {partition} AND created > {since}
                                GROUP BY status
                                ORDER BY COUNT(*) DESC
                                "#
                            ))
                            .fetch_all::<(i16, u64)>()
                            .await?)
                    },
                    async {
                        Ok(state
                            .clickhouse
                            .client()
                            .query(&format!(
                                r#"
                                SELECT
                                    path,
                                    COUNT(*) AS total,
                                    avg(time),
                                    quantile(0.95)(time)
                                FROM requests
                                WHERE {partition} AND created > {since}
                                GROUP BY path
                                HAVING total > 10
                                ORDER BY quantile(0.95)(time) DESC
                                LIMIT 15
                                "#
                            ))
                            .fetch_all::<(String, u64, f64, f64)>()
                            .await?)
                    },
                    async {
                        Ok(state
                            .clickhouse
                            .client()
                            .query(&format!(
                                r#"
                                SELECT
                                    IPv6NumToString(ip),
                                    COUNT(*),
                                    countIf(status >= 400),
                                    uniqExact(user_agent),
                                    toString(any(country))
                                FROM requests
                                WHERE {partition} AND created > {since}
                                GROUP BY ip
                                ORDER BY COUNT(*) DESC
                                LIMIT 15
                                "#
                            ))
                            .fetch_all::<(String, u64, u64, u64, Option<String>)>()
                            .await?)
                    },
                    async {
                        Ok(state
                            .clickhouse
                            .client()
                            .query(&format!(
                                r#"
                                SELECT user_agent, COUNT(*), uniqExact(ip)
                                FROM requests
                                WHERE {partition} AND created > {since}
                                GROUP BY user_agent
                                ORDER BY COUNT(*) DESC
                                LIMIT 15
                                "#
                            ))
                            .fetch_all::<(String, u64, u64)>()
                            .await?)
                    },
                    async {
                        Ok(state
                            .clickhouse
                            .client()
                            .query(&format!(
                                r#"
                                SELECT assumeNotNull(origin), COUNT(*), uniqExact(ip)
                                FROM requests
                                WHERE {partition} AND created > {since} AND origin IS NOT NULL
                                GROUP BY origin
                                ORDER BY COUNT(*) DESC
                                LIMIT 15
                                "#
                            ))
                            .fetch_all::<(String, u64, u64)>()
                            .await?)
                    },
                    async {
                        Ok(state
                            .clickhouse
                            .client()
                            .query(&format!(
                                r#"
                                SELECT toString(assumeNotNull(country)), COUNT(*), uniqExact(ip)
                                FROM requests
                                WHERE {partition} AND created > {since} AND country IS NOT NULL
                                GROUP BY country
                                ORDER BY COUNT(*) DESC
                                LIMIT 15
                                "#
                            ))
                            .fetch_all::<(String, u64, u64)>()
                            .await?)
                    },
                )?;

                let top = |rows: Vec<(String, u64, u64)>| {
                    rows.into_iter()
                        .map(|(label, total, unique_ips)| TopEntry {
                            label,
                            total,
                            unique_ips,
                        })
                        .collect::<Vec<_>>()
                };

                Ok::<_, anyhow::Error>(Requests {
                    window: window.into(),

                    total: totals.0,
                    unique_ips: totals.1,
                    errors: totals.2,
                    avg_time: totals.3,
                    p95_time: totals.4,

                    timeline: timeline
                        .into_iter()
                        .map(|(time, total, unique_ips, errors, avg_time)| TimelinePoint {
                            time: time as i64,
                            total,
                            unique_ips,
                            errors,
                            avg_time,
                        })
                        .collect(),
                    statuses: statuses
                        .into_iter()
                        .map(|(status, total)| StatusCount { status, total })
                        .collect(),
                    slowest: slowest
                        .into_iter()
                        .map(|(path, total, avg_time, p95_time)| SlowPath {
                            path,
                            total,
                            avg_time,
                            p95_time,
                        })
                        .collect(),
                    ips: ips
                        .into_iter()
                        .map(|(ip, total, errors, user_agents, country)| TopIp {
                            ip,
                            total,
                            errors,
                            user_agents,
                            country,
                        })
                        .collect(),
                    user_agents: top(user_agents),
                    origins: top(origins),
                    countries: top(countries),
                })
            })
            .await?;

        ApiResponse::new_serialized(Response {
            success: true,
            requests,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
