use super::{ApiError, GetState, State};
use axum::{
    http::{HeaderMap, StatusCode},
    routing::get,
};
use utoipa_axum::router::OpenApiRouter;

mod github;
mod internal;
mod organization;
mod user;
mod v1;
mod v2;
mod v3;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .route(
            "/",
            get(|| async move {
                let mut headers = HeaderMap::new();

                headers.insert("Content-Type", "text/html".parse().unwrap());

                (
                    StatusCode::OK,
                    headers,
                    include_str!("../../../static/api.html"),
                )
            }),
        )
        .nest("/v1", v1::router(state))
        .nest("/v2", v2::router(state))
        .nest("/v3", v3::router(state))
        .nest("/organization", organization::router(state))
        .nest("/github", github::router(state))
        .nest("/internal", internal::router(state))
        .nest("/user", user::router(state))
        .with_state(state.clone())
}
