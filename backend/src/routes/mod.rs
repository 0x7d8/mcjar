use crate::{models::organization::Organization, requests::RateLimitError, response::ApiResponse};
use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;

mod api;
mod files;
mod index;

#[derive(ToSchema, Serialize)]
pub struct ApiError<'a> {
    #[schema(default = false)]
    pub success: bool,
    pub errors: &'a [&'a str],
}

impl<'a> ApiError<'a> {
    pub fn new(errors: &'a [&'a str]) -> Self {
        Self {
            success: false,
            errors,
        }
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

#[derive(ToSchema, Serialize)]
pub struct ApiErrorV3<'a> {
    pub errors: &'a [&'a str],
}

impl<'a> ApiErrorV3<'a> {
    pub fn new(errors: &'a [&'a str]) -> Self {
        Self { errors }
    }

    pub fn new_strings_value(errors: Vec<String>) -> serde_json::Value {
        serde_json::json!({
            "errors": errors,
        })
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

pub struct AppState {
    pub start_time: Instant,
    pub version: String,

    pub database: Arc<crate::database::Database>,
    pub clickhouse: Arc<crate::clickhouse::Clickhouse>,
    pub cache: Arc<crate::cache::Cache>,
    pub requests: crate::requests::RequestLogger,
    pub files: crate::files::FileCache,
    pub env: Arc<crate::env::Env>,
    pub s3: Arc<crate::s3::S3>,
}

pub type State = Arc<AppState>;
pub type GetState = axum::extract::State<State>;
pub type GetData = axum::extract::Extension<Arc<Mutex<serde_json::Value>>>;

async fn handle_api_request(state: GetState, req: Request, next: Next) -> Response<Body> {
    let mut organization: Option<Organization> = None;
    if let Some(authorization) = req.headers().get("Authorization")
        && let Ok(authorization) = authorization.to_str()
        && authorization.len() == 64
        && let Ok(org) = Organization::by_key(&state.database, &state.cache, authorization).await
    {
        organization = org;
    }

    let (parts, body) = req.into_parts();
    let request_id = state.requests.log(&parts, organization.as_ref()).await;

    let (request_id, ratelimit) = match request_id {
        Ok(request_id) => request_id,
        Err(RateLimitError::Requests(ratelimit)) => {
            return ApiResponse::error("too many requests")
                .with_status(StatusCode::TOO_MANY_REQUESTS)
                .with_header("X-RateLimit-Limit", &ratelimit.limit.to_string())
                .with_header(
                    "X-RateLimit-Remaining",
                    &(ratelimit.limit - ratelimit.hits).max(0).to_string(),
                )
                .with_header("X-RateLimit-Reset", &ratelimit.reset.to_string())
                .with_header("Retry-After", &ratelimit.reset.to_string())
                .into_response();
        }
        Err(RateLimitError::Bandwidth { limit, used, reset }) => {
            return ApiResponse::error("daily bandwidth limit exceeded")
                .with_status(StatusCode::TOO_MANY_REQUESTS)
                .with_header("X-Bandwidth-Limit", &limit.to_string())
                .with_header("X-Bandwidth-Remaining", &(limit - used).max(0).to_string())
                .with_header("X-Bandwidth-Reset", &reset.to_string())
                .with_header("Retry-After", &reset.to_string())
                .into_response();
        }
    };

    let mut headers = HeaderMap::new();
    if let Some(request_id) = &request_id {
        headers.insert("X-Request-ID", request_id.parse().unwrap());
    }

    if let Some(ratelimit) = ratelimit {
        headers.insert(
            "X-RateLimit-Limit",
            ratelimit.limit.to_string().parse().unwrap(),
        );
        headers.insert(
            "X-RateLimit-Remaining",
            (ratelimit.limit - ratelimit.hits)
                .max(0)
                .to_string()
                .parse()
                .unwrap(),
        );
        headers.insert(
            "X-RateLimit-Reset",
            ratelimit.reset.to_string().parse().unwrap(),
        );
    }

    let mut req = Request::from_parts(parts, body);
    let data = Arc::new(Mutex::new(serde_json::Value::Null));
    req.extensions_mut().insert(data.clone());
    req.extensions_mut().insert(organization);

    let start = Instant::now();
    let mut response = next.run(req).await;

    if let Some(request_id) = request_id {
        let data = if let serde_json::Value::Object(data) = data.lock().unwrap().take() {
            Some(serde_json::Value::Object(data))
        } else {
            None
        };

        state
            .requests
            .finish(
                request_id,
                response.status().as_u16() as i16,
                start.elapsed().as_millis() as i32,
                data,
                None,
            )
            .await;
    }

    response.headers_mut().extend(headers);
    if let Some(server_name) = state.env.server_name.as_ref() {
        response
            .headers_mut()
            .insert("X-Server-Name", server_name.parse().unwrap());
    }

    response
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/api", api::router(state))
        .nest("/index", index::router(state))
        .nest("/files", files::router(state))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            handle_api_request,
        ))
        .with_state(state.clone())
}
