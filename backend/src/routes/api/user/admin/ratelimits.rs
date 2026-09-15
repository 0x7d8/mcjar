use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use crate::{
        models::{Pagination, PaginationParamsWithSearch},
        requests::RateLimitBucket,
        response::{ApiResponse, ApiResponseResult},
        routes::{ApiError, GetState, api::user::admin::NodeStatus},
    };
    use axum::http::StatusCode;
    use axum_extra::extract::Query;
    use indexmap::IndexMap;
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[schema(rename_all = "camelCase")]
    struct RateLimit {
        ip: String,
        bucket: RateLimitBucket,

        hits: i64,
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
        ratelimits: Pagination<RateLimit>,
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
            description = "Filter entries by IP address",
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

        let local = crate::requests::ratelimit_snapshot(&state.cache).await?;
        let fanned = super::super::fanout_with_local(&state, "/ratelimits", local).await?;

        let mut merged: IndexMap<(String, RateLimitBucket), RateLimit> = IndexMap::new();
        for (node, snapshots) in fanned.data {
            for snapshot in snapshots {
                let entry = merged
                    .entry((snapshot.ip.clone(), snapshot.bucket))
                    .or_insert_with(|| RateLimit {
                        ip: snapshot.ip,
                        bucket: snapshot.bucket,
                        hits: 0,
                        limit: snapshot.limit,
                        reset: snapshot.reset,
                        nodes: IndexMap::new(),
                    });

                entry.hits += snapshot.hits;
                entry.reset = entry.reset.min(snapshot.reset);
                entry.nodes.insert(node.clone(), snapshot.hits);
            }
        }

        let mut ratelimits: Vec<RateLimit> = merged
            .into_values()
            .filter(|entry| {
                params
                    .search
                    .as_ref()
                    .is_none_or(|search| entry.ip.contains(search.as_str()))
            })
            .collect();
        ratelimits.sort_by(|a, b| b.hits.cmp(&a.hits).then_with(|| a.ip.cmp(&b.ip)));

        let total = ratelimits.len() as i64;
        let offset = ((params.page - 1) * params.per_page).min(total) as usize;

        ApiResponse::new_serialized(Response {
            success: true,
            nodes: fanned.nodes,
            ratelimits: Pagination {
                total,
                per_page: params.per_page,
                page: params.page,
                data: ratelimits
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
