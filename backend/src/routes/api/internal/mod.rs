use super::{ApiError, GetState, State};
use crate::response::{ApiResponse, ApiResponseResult};
use axum::{
    body::Body, extract::Request, http::StatusCode, middleware::Next, response::Response,
    routing::get,
};
use utoipa_axum::router::OpenApiRouter;

async fn auth(state: GetState, req: Request, next: Next) -> Result<Response, StatusCode> {
    let unauthorized = || {
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::to_string(&ApiError::new(&["unauthorized"])).unwrap(),
            ))
            .unwrap()
    };

    let Some(secret) = state.env.node_secret.as_deref() else {
        return Ok(unauthorized());
    };

    let presented = req
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    if presented != Some(secret) {
        return Ok(unauthorized());
    }

    Ok(next.run(req).await)
}

async fn ratelimits(state: GetState) -> ApiResponseResult {
    ApiResponse::new_serialized(crate::requests::ratelimit_snapshot(&state.cache).await?).ok()
}

async fn bandwidth(state: GetState) -> ApiResponseResult {
    ApiResponse::new_serialized(crate::requests::bandwidth_snapshot(&state.cache).await?).ok()
}

async fn system(state: GetState) -> ApiResponseResult {
    ApiResponse::new_serialized(crate::nodes::SystemSnapshot::capture(&state).await?).ok()
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .route("/ratelimits", get(ratelimits))
        .route("/bandwidth", get(bandwidth))
        .route("/system", get(system))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state.clone())
}
