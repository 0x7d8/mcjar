use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use crate::{
        response::{ApiResponse, ApiResponseResult},
        routes::{ApiError, GetState},
    };
    use axum::http::StatusCode;
    use axum_extra::extract::Query;
    use serde::{Deserialize, Serialize};
    use sqlx::Row;
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
    struct BandwidthPoint {
        time: i64,
        bytes: u64,
        requests: u64,
        cache_hits: u64,
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[schema(rename_all = "camelCase")]
    struct Breakdown {
        label: String,
        requests: i64,
        unique_ips: i64,
        bytes: i64,
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[schema(rename_all = "camelCase")]
    struct Files {
        window: compact_str::CompactString,

        bytes_sent: u64,
        requests: u64,
        cache_hits: u64,
        cache_hit_rate: f64,
        avg_time: f64,

        #[schema(inline)]
        timeline: Vec<BandwidthPoint>,
        #[schema(inline)]
        roots: Vec<Breakdown>,
        #[schema(inline)]
        kinds: Vec<Breakdown>,
        #[schema(inline)]
        extensions: Vec<Breakdown>,
        #[schema(inline)]
        top: Vec<Breakdown>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        success: bool,

        #[schema(inline)]
        files: Files,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = inline(ApiError)),
    ), params(
        (
            "window" = String, Query,
            description = "The time window the live figures cover",
            example = "day",
        ),
    ))]
    pub async fn route(state: GetState, params: Query<Params>) -> ApiResponseResult {
        let Some((window, bucket, partition_days)) = super::super::requests::WINDOWS
            .iter()
            .find(|(name, _, _)| *name == params.window.as_str())
            .copied()
        else {
            return ApiResponse::error("invalid window")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        };

        let files = state
            .cache
            .cached(&format!("stats::admin::files::{window}"), 60, || async {
                let since = format!("NOW() - INTERVAL 1 {}", window.to_uppercase());
                let partition =
                    format!("_partition_date >= toDate(now() - INTERVAL {partition_days} DAY)");

                let (totals, timeline, breakdowns) = tokio::try_join!(
                    async {
                        Ok::<_, anyhow::Error>(
                            state
                                .clickhouse
                                .client()
                                .query(&format!(
                                    r#"
                                    SELECT
                                        toUInt64(SUM(bytes_sent)),
                                        COUNT(*),
                                        countIf(cache_hit),
                                        avg(time)
                                    FROM file_requests
                                    WHERE {partition} AND created > {since}
                                    "#
                                ))
                                .fetch_one::<(u64, u64, u64, f64)>()
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
                                    toUInt64(SUM(bytes_sent)),
                                    COUNT(*),
                                    countIf(cache_hit)
                                FROM file_requests
                                WHERE {partition} AND created > {since}
                                GROUP BY time
                                ORDER BY time ASC
                                "#
                            ))
                            .fetch_all::<(u32, u64, u64, u64)>()
                            .await?)
                    },
                    async {
                        let rows = sqlx::query(
                            r#"
                            SELECT 'root' AS dimension, ch_file_stats.root AS label,
                                SUM(ch_file_stats.total_requests)::bigint, SUM(ch_file_stats.unique_ips)::bigint, SUM(ch_file_stats.total_bytes)::bigint
                            FROM ch_file_stats
                            GROUP BY ch_file_stats.root
                            UNION ALL
                            SELECT 'kind', ch_file_stats.kind,
                                SUM(ch_file_stats.total_requests)::bigint, SUM(ch_file_stats.unique_ips)::bigint, SUM(ch_file_stats.total_bytes)::bigint
                            FROM ch_file_stats
                            GROUP BY ch_file_stats.kind
                            UNION ALL
                            SELECT 'extension', ch_file_stats.extension,
                                SUM(ch_file_stats.total_requests)::bigint, SUM(ch_file_stats.unique_ips)::bigint, SUM(ch_file_stats.total_bytes)::bigint
                            FROM ch_file_stats
                            WHERE ch_file_stats.extension <> ''
                            GROUP BY ch_file_stats.extension
                            UNION ALL
                            SELECT * FROM (
                                SELECT 'path', ch_file_stats.path,
                                    SUM(ch_file_stats.total_requests)::bigint, SUM(ch_file_stats.unique_ips)::bigint, SUM(ch_file_stats.total_bytes)::bigint
                                FROM ch_file_stats
                                WHERE ch_file_stats.kind = 'file'
                                GROUP BY ch_file_stats.path
                                ORDER BY 3 DESC
                                LIMIT 25
                            ) top_paths
                            "#,
                        )
                        .fetch_all(state.database.read())
                        .await?;

                        Ok(rows)
                    },
                )?;

                let mut roots = Vec::new();
                let mut kinds = Vec::new();
                let mut extensions = Vec::new();
                let mut top = Vec::new();

                for row in breakdowns.iter() {
                    let dimension: String = row.try_get(0)?;
                    let entry = Breakdown {
                        label: row.try_get(1)?,
                        requests: row.try_get(2)?,
                        unique_ips: row.try_get(3)?,
                        bytes: row.try_get(4)?,
                    };

                    match dimension.as_str() {
                        "root" => roots.push(entry),
                        "kind" => kinds.push(entry),
                        "extension" => extensions.push(entry),
                        _ => top.push(entry),
                    }
                }

                for breakdown in [&mut roots, &mut kinds, &mut extensions, &mut top] {
                    breakdown.sort_by_key(|entry| std::cmp::Reverse(entry.requests));
                }

                Ok::<_, anyhow::Error>(Files {
                    window: window.into(),

                    bytes_sent: totals.0,
                    requests: totals.1,
                    cache_hits: totals.2,
                    cache_hit_rate: if totals.1 == 0 {
                        0.0
                    } else {
                        totals.2 as f64 / totals.1 as f64
                    },
                    avg_time: totals.3,

                    timeline: timeline
                        .into_iter()
                        .map(|(time, bytes, requests, cache_hits)| BandwidthPoint {
                            time: time as i64,
                            bytes,
                            requests,
                            cache_hits,
                        })
                        .collect(),
                    roots,
                    kinds,
                    extensions,
                    top,
                })
            })
            .await?;

        ApiResponse::new_serialized(Response {
            success: true,
            files,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
