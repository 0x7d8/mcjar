use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use crate::{
        nodes::SystemSnapshot,
        response::{ApiResponse, ApiResponseResult},
        routes::GetState,
    };
    use chrono::NaiveDateTime;
    use serde::Serialize;
    use std::collections::HashMap;
    use utoipa::ToSchema;

    type Reported = (Option<i64>, Option<SystemSnapshot>, Option<String>);

    #[derive(ToSchema, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[schema(rename_all = "camelCase")]
    struct ApiNode {
        name: compact_str::CompactString,
        url: compact_str::CompactString,
        version: compact_str::CompactString,

        local: bool,
        alive: bool,
        latency: Option<i64>,

        last_seen: NaiveDateTime,
        created: NaiveDateTime,

        system: Option<SystemSnapshot>,
        error: Option<String>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        success: bool,

        #[schema(inline)]
        nodes: Vec<ApiNode>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState) -> ApiResponseResult {
        let local_name = state.env.server_name.as_deref();

        let (registered, fanned) = tokio::try_join!(state.nodes.all(), async {
            state.nodes.fanout::<SystemSnapshot>("/system").await
        })?;

        let mut reported: HashMap<compact_str::CompactString, Reported> = fanned
                .into_iter()
                .map(|result| (result.node, (result.latency, result.data, result.error)))
                .collect();

        let local_system = SystemSnapshot::capture(&state).await?;

        let mut nodes = Vec::with_capacity(registered.len());
        for node in registered {
            let local = Some(node.name.as_str()) == local_name;
            let (latency, system, error) = if local {
                (Some(0), Some(local_system.clone()), None)
            } else {
                reported.remove(&node.name).unwrap_or((None, None, None))
            };

            nodes.push(ApiNode {
                alive: node.alive(),
                name: node.name,
                url: node.url,
                version: node.version,
                local,
                latency,
                last_seen: node.last_seen,
                created: node.created,
                system,
                error,
            });
        }

        if local_name.is_none() {
            nodes.push(ApiNode {
                name: compact_str::CompactString::const_new(super::super::UNNAMED_NODE),
                url: compact_str::CompactString::const_new(""),
                version: compact_str::CompactString::new(&state.version),
                local: true,
                alive: true,
                latency: Some(0),
                last_seen: chrono::Utc::now().naive_utc(),
                created: chrono::Utc::now().naive_utc(),
                system: Some(local_system),
                error: None,
            });
        }

        ApiResponse::new_serialized(Response {
            success: true,
            nodes,
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
