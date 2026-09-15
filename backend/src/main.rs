use api::{
    models::r#type::ServerType,
    response::{ApiResponse, ApiResponseResult},
    routes::{ApiError, ApiErrorV3, GetState},
};
use axum::{
    ServiceExt,
    body::Body,
    extract::{Path, Request},
    http::{Method, StatusCode},
    middleware::Next,
    response::Response,
    routing::get,
};
use compact_str::ToCompactString;
use include_dir::{Dir, include_dir};
use sentry_tower::{NewSentryLayer, SentryHttpLayer};
use sha2::Digest;
use std::{borrow::Cow, sync::Arc, time::Instant};
use tower::Layer;
use tower_cookies::CookieManagerLayer;
use tower_http::{
    catch_panic::CatchPanicLayer, cors::CorsLayer, normalize_path::NormalizePathLayer,
    timeout::RequestBodyTimeoutLayer,
};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa_axum::router::OpenApiRouter;

#[cfg(target_arch = "x86_64")]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_COMMIT: &str = env!("CARGO_GIT_COMMIT");

const FRONTEND_ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../frontend/lib/client");
const SPA_FALLBACK: &str = "__spa-fallback.html";

const REQUEST_BODY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

fn content_type_for(extension: Option<&std::ffi::OsStr>) -> &'static str {
    match extension {
        Some(ext) if ext == "js" => "application/javascript",
        Some(ext) if ext == "css" => "text/css",
        Some(ext) if ext == "html" => "text/html",
        Some(ext) if ext == "json" => "application/json",
        Some(ext) if ext == "svg" => "image/svg+xml",
        Some(ext) if ext == "png" => "image/png",
        Some(ext) if ext == "ico" => "image/x-icon",
        Some(ext) if ext == "xml" => "application/xml",
        Some(ext) if ext == "txt" => "text/plain",
        Some(ext) if ext == "sh" => "text/x-shellscript",
        _ => "application/octet-stream",
    }
}

fn inject_head(html: &str, meta: Vec<(&str, String)>, version: &str) -> String {
    let mut head = String::new();

    for (key, value) in meta {
        let attribute = if key.starts_with("og:") {
            "property"
        } else {
            "name"
        };

        head.push_str(&format!("<meta {attribute}=\"{key}\" content=\"{value}\">"));
    }

    head.push_str(&format!(
        "<script>window.mcjars_version = \"{version}\"</script>"
    ));

    html.replacen("</head>", &format!("{head}</head>"), 1)
}
fn render_page(file_path: &str, meta: Vec<(&str, String)>, state: &GetState) -> ApiResponseResult {
    let file = FRONTEND_ASSETS
        .get_file(file_path)
        .or_else(|| FRONTEND_ASSETS.get_file(SPA_FALLBACK))
        .unwrap();

    let content = inject_head(file.contents_utf8().unwrap(), meta, &state.version);

    ApiResponse::new(Body::from(content))
        .with_header("Content-Type", "text/html")
        .ok()
}

fn handle_panic(_err: Box<dyn std::any::Any + Send + 'static>) -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("Content-Type", "application/json")
        .body(Body::from(
            ApiError::new(&["internal server error"])
                .to_value()
                .to_string(),
        ))
        .unwrap()
}

async fn handle_request(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let ip = api::utils::extract_ip(req.headers())
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    tracing::info!(
        ip,
        path = req.uri().path(),
        query = req.uri().query().unwrap_or_default(),
        "http {}",
        req.method().to_string().to_lowercase(),
    );

    Ok(api::response::IS_V3
        .scope(req.uri().path().starts_with("/api/v3/"), async {
            api::response::ACCEPT_HEADER
                .scope(api::response::accept_from_headers(req.headers()), async {
                    next.run(req).await
                })
                .await
        })
        .await)
}

async fn handle_postprocessing(req: Request, next: Next) -> Result<Response, StatusCode> {
    let is_v3 = req.uri().path().starts_with("/api/v3/");
    let if_none_match = req.headers().get("If-None-Match").cloned();
    let mut response = next.run(req).await;

    if let Some(content_type) = response.headers().get("Content-Type")
        && content_type
            .to_str()
            .map(|c| c.starts_with("text/plain"))
            .unwrap_or(false)
        && response.status().is_client_error()
        && response.status() != StatusCode::NOT_FOUND
    {
        let (mut parts, body) = response.into_parts();

        let text_body = String::from_utf8(
            axum::body::to_bytes(body, usize::MAX)
                .await
                .unwrap()
                .into_iter()
                .by_ref()
                .collect::<Vec<u8>>(),
        )
        .unwrap();

        parts
            .headers
            .insert("Content-Type", "application/json".parse().unwrap());

        if is_v3 {
            response = Response::from_parts(
                parts,
                Body::from(ApiErrorV3::new(&[&text_body]).to_value().to_string()),
            );
        } else {
            response = Response::from_parts(
                parts,
                Body::from(ApiError::new(&[&text_body]).to_value().to_string()),
            );
        }
    }

    if !response.headers().contains_key("ETag") {
        let (mut parts, body) = response.into_parts();
        let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        let mut hash = sha2::Sha256::new();
        hash.update(body_bytes.as_ref());
        let hash = hex::encode(hash.finalize());

        parts.headers.insert("ETag", hash.parse().unwrap());

        if if_none_match == Some(hash.parse().unwrap()) {
            let mut cached_response = Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .body(Body::empty())
                .unwrap();

            for (key, value) in parts.headers.iter() {
                cached_response.headers_mut().insert(key, value.clone());
            }

            return Ok(cached_response);
        }

        Ok(Response::from_parts(parts, Body::from(body_bytes)))
    } else {
        Ok(response)
    }
}

#[tokio::main]
async fn main() {
    let (env, _tracing_guard) = match api::env::Env::parse() {
        Ok((env, tracing_guard)) => (env, tracing_guard),
        Err(err) => {
            eprintln!("failed to parse environment: {err:#?}");
            std::process::exit(1);
        }
    };

    let _guard = sentry::init((
        env.sentry_url.clone(),
        sentry::ClientOptions::default()
            .release(format!("{VERSION}:{GIT_COMMIT}"))
            .server_name(
                env.server_name
                    .clone()
                    .map_or(Cow::Borrowed("mcjars"), |s| s.into()),
            )
            .traces_sample_rate(1.0),
    ));

    let s3 = Arc::new(api::s3::S3::new(env.clone()).await);
    let database = Arc::new(api::database::Database::new(env.clone()).await);
    let clickhouse =
        Arc::new(api::clickhouse::Clickhouse::new(env.clone(), database.clone()).await);
    let cache = Arc::new(api::cache::Cache::new(env.clone()).await);

    let state = Arc::new(api::routes::AppState {
        start_time: Instant::now(),
        version: format!("{VERSION}:{GIT_COMMIT}"),

        database: database.clone(),
        clickhouse: clickhouse.clone(),
        cache: cache.clone(),
        requests: api::requests::RequestLogger::new(
            database.clone(),
            clickhouse.clone(),
            cache.clone(),
        ),
        files: api::files::FileCache::new(database.clone(), env.clone()).await,
        nodes: api::nodes::NodeClient::new(database.clone(), env.clone()),
        env,
        s3,
    });

    {
        let state = state.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                if let Err(err) = state.requests.process().await {
                    tracing::error!("failed to process requests: {:?}", err);
                    sentry_anyhow::capture_anyhow(&err);
                }
            }
        });
    }

    {
        let state = state.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                if let Err(err) = state.files.process().await {
                    tracing::error!("failed to process files: {:?}", err);
                    sentry_anyhow::capture_anyhow(&err);
                }
            }
        });
    }

    if state.nodes.name().is_some() && state.env.node_url.is_some() {
        let state = state.clone();

        tokio::spawn(async move {
            loop {
                if let Err(err) = state.nodes.heartbeat(&state.version).await {
                    tracing::error!("failed to heartbeat node: {:?}", err);
                    sentry_anyhow::capture_anyhow(&err);
                }

                tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            }
        });
    }

    let app =
        OpenApiRouter::new()
            .merge(api::routes::router(&state))
            .route(
                "/icons/{type}",
                get(|state: GetState, Path::<ServerType>(r#type)| async move {
                    ApiResponse::new(Body::empty())
                        .with_status(StatusCode::FOUND)
                        .with_header(
                            "Location",
                            &format!(
                                "{}/icons/{}.png",
                                state.env.s3_url,
                                r#type.to_string().to_lowercase()
                            ),
                        )
                        .ok()
                }),
            )
            .route(
                "/download/{project}/{version}/{project_version}/{installer_version}",
                get(
                    |Path::<(String, String, String, String)>((
                        project,
                        version,
                        project_version,
                        installer_version,
                    ))| async move {
                        let response = match project.as_str() {
                            "fabric" => reqwest::get(
                                format!(
                                    "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
                                    version,
                                    project_version,
                                    installer_version.replace(".jar", "")
                                )
                                .as_str(),
                            )
                            .await,
                            "legacy-fabric" => reqwest::get(
                                format!(
                                    "https://meta.legacyfabric.net/v2/versions/loader/{}/{}/{}/server/jar",
                                    version,
                                    project_version,
                                    installer_version.replace(".jar", "")
                                )
                                .as_str(),
                            )
                            .await,
                            _ => return ApiResponse::error("project not supported")
                                .with_status(StatusCode::NOT_FOUND)
                                .ok(),
                        };

                        let response = match response {
                            Ok(response) => response,
                            Err(_) => {
                                return ApiResponse::error("error fetching build")
                                    .with_status(StatusCode::NOT_FOUND)
                                    .ok();
                            }
                        };

                        if !response.status().is_success() {
                            return ApiResponse::error("build not found")
                                .with_status(StatusCode::NOT_FOUND)
                                .ok();
                        }

                        let content_type = response
                            .headers()
                            .get("Content-Type")
                            .unwrap()
                            .to_str()?
                            .to_compact_string();
                        let content_disposition = response
                            .headers()
                            .get("Content-Disposition")
                            .unwrap()
                            .to_str()?
                            .to_compact_string();

                        ApiResponse::new(Body::from(response.bytes().await?))
                            .with_header("Content-Type", &content_type)
                            .with_header("Content-Disposition", &content_disposition)
                            .ok()
                    },
                ),
            )
            .route("/", get(|state: GetState| async move {
                let meta = Vec::from([
                    ("description", "MCJars is a service that provides Minecraft server owners with the ability to download server jars and other files with ease. Not affiliated with Mojang AB.".to_string()),
                    ("og:description", "MCJars is a service that provides Minecraft server owners with the ability to download server jars and other files with ease. Not affiliated with Mojang AB.".to_string()),
                    ("og:title", "MCJars".to_string()),
                    ("og:image", format!("{}/icons/vanilla.png", state.env.s3_url)),
                    ("og:url", state.env.app_frontend_url.clone()),
                ]);

                render_page("index.html", meta, &state)
            }))
            .route("/lookup", get(|state: GetState| async move {
                let meta = Vec::from([
                    ("description", "Lookup Minecraft server jars and configs by their hash. Not affiliated with Mojang AB.".to_string()),
                    ("og:description", "Lookup Minecraft server jars and configs by their hash. Not affiliated with Mojang AB.".to_string()),
                    ("og:title", "MCJars | Reverse Lookup".to_string()),
                    ("og:image", format!("{}/icons/vanilla.png", state.env.s3_url)),
                    ("og:url", format!("{}/lookup", state.env.app_frontend_url)),
                ]);

                render_page("lookup/index.html", meta, &state)
            }))
            .route("/configs", get(|state: GetState| async move {
                let meta = Vec::from([
                    ("description", "Browse the default configuration files shipped by every Minecraft server software MCJars tracks. Not affiliated with Mojang AB.".to_string()),
                    ("og:description", "Browse the default configuration files shipped by every Minecraft server software MCJars tracks. Not affiliated with Mojang AB.".to_string()),
                    ("og:title", "MCJars | Configs".to_string()),
                    ("og:image", format!("{}/icons/vanilla.png", state.env.s3_url)),
                    ("og:url", format!("{}/configs", state.env.app_frontend_url)),
                ]);

                render_page("configs/index.html", meta, &state)
            }))
            .route("/job-status", get(|state: GetState| async move {
                let meta = Vec::from([
                    ("description", "View the job status for MCJars. Not affiliated with Mojang AB.".to_string()),
                    ("og:description", "View the job status for MCJars. Not affiliated with Mojang AB.".to_string()),
                    ("og:title", "MCJars | Job Status".to_string()),
                    ("og:image", format!("{}/icons/vanilla.png", state.env.s3_url)),
                    ("og:url", format!("{}/job-status", state.env.app_frontend_url)),
                ]);

                render_page(SPA_FALLBACK, meta, &state)
            }))
            .route("/organizations", get(|state: GetState| async move {
                let meta = Vec::from([
                    ("description", "MCJars is a service that provides Minecraft server owners with the ability to download server jars and other files with ease. Not affiliated with Mojang AB.".to_string()),
                    ("og:description", "MCJars is a service that provides Minecraft server owners with the ability to download server jars and other files with ease. Not affiliated with Mojang AB.".to_string()),
                    ("og:title", "MCJars".to_string()),
                    ("og:image", format!("{}/icons/vanilla.png", state.env.s3_url)),
                    ("og:url", format!("{}/organizations", state.env.app_frontend_url)),
                ]);

                render_page(SPA_FALLBACK, meta, &state)
            }))
            .route("/{type}/versions", get(|state: GetState, Path::<ServerType>(r#type)| async move {
                let types = ServerType::all(&state.database, &state.cache, &state.env).await?;
                let data = types
                    .iter()
                    .find(|(k, _)| **k == r#type)
                    .unwrap();

                let builds = data.1.builds;
                let versions = if data.1.versions.minecraft == 0 {
                    data.1.versions.project
                } else {
                    data.1.versions.minecraft
                };

                let meta = Vec::from([
                    ("description", format!("Download the latest {} server builds with ease. Browse {} builds for {} different versions on our website. Not affiliated with Mojang AB.", data.1.name, builds, versions).to_string()),
                    ("og:description", format!("Download the latest {} server builds with ease. Browse {} builds for {} different versions on our website. Not affiliated with Mojang AB.", data.1.name, builds, versions).to_string()),
                    ("og:title", format!("MCJars | {} Versions", data.1.name).to_string()),
                    ("og:image", format!("{}/icons/{}.png", state.env.s3_url, r#type.to_string().to_lowercase())),
                    ("og:url", format!("{}/{type}/versions", state.env.app_frontend_url)),
                ]);

                render_page(&format!("{type}/versions/index.html"), meta, &state)
            }))
            .route("/{type}/config", get(|state: GetState, Path::<ServerType>(r#type)| async move {
                let data = r#type.infos(&state.env);

                let meta = Vec::from([
                    ("description", format!("Browse and compare the {} configuration files across builds and versions. Not affiliated with Mojang AB.", data.name).to_string()),
                    ("og:description", format!("Browse and compare the {} configuration files across builds and versions. Not affiliated with Mojang AB.", data.name).to_string()),
                    ("og:title", format!("MCJars | {} Configs", data.name).to_string()),
                    ("og:image", format!("{}/icons/{}.png", state.env.s3_url, r#type.to_string().to_lowercase())),
                    ("og:url", format!("{}/{type}/config", state.env.app_frontend_url)),
                ]);

                render_page(&format!("{type}/config/index.html"), meta, &state)
            }))
            .route("/{type}/statistics", get(|state: GetState, Path::<ServerType>(r#type)| async move {
                let data = r#type.infos(&state.env);

                let meta = Vec::from([
                    ("description", format!("View the latest statistics for {}. Not affiliated with Mojang AB.", data.name).to_string()),
                    ("og:description", format!("View the latest statistics for {}. Not affiliated with Mojang AB.", data.name).to_string()),
                    ("og:title", format!("MCJars | {} Statistics", data.name).to_string()),
                    ("og:image", format!("{}/icons/{}.png", state.env.s3_url, r#type.to_string().to_lowercase())),
                    ("og:url", format!("{}/{type}/statistics", state.env.app_frontend_url)),
                ]);

                render_page(&format!("{type}/statistics/index.html"), meta, &state)
            }))
            .route("/sitemap.xml", get(|state: GetState| async move {
                let mut sitemap = "
                <?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">
                <url><loc>https://mcjars.app</loc></url>
                <url><loc>https://mcjars.app/lookup</loc></url>
                <url><loc>https://mcjars.app/configs</loc></url>
                ".trim().to_string();

                let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

                for r#type in ServerType::variants(&state.env) {
                    sitemap.push_str(&format!(
                        "<url><loc>https://mcjars.app/{type}/versions</loc><lastmod>{now}</lastmod></url>"
                    ));
                    sitemap.push_str(&format!(
                        "<url><loc>https://mcjars.app/{type}/statistics</loc><lastmod>{now}</lastmod></url>"
                    ));
                    sitemap.push_str(&format!(
                        "<url><loc>https://mcjars.app/{type}/config</loc><lastmod>{now}</lastmod></url>"
                    ));
                }

                sitemap.push_str("</urlset>");

                ApiResponse::new(Body::from(sitemap)).with_header("Content-Type", "application/xml").ok()
            }))
            .fallback(|state: GetState, req: Request<Body>| async move {
                if req.uri().path().starts_with("/api") {
                    return ApiResponse::error("route not found")
                        .with_status(StatusCode::NOT_FOUND)
                        .ok();
                }

                let path = &req.uri().path()[1..];

                if !path.is_empty()
                    && let Some(file) = FRONTEND_ASSETS.get_file(path)
                {
                    let extension = file.path().extension();

                    if extension == Some("html".as_ref()) && let Some(contents_utf8) = file.contents_utf8() {
                        let content =
                            inject_head(contents_utf8, Vec::new(), &state.version);

                        return ApiResponse::new(Body::from(content))
                            .with_header("Content-Type", "text/html")
                            .ok();
                    }

                    return ApiResponse::new(Body::from(file.contents()))
                        .with_header("Content-Type", content_type_for(extension))
                        .ok();
                }

                let route_index = if path.is_empty() {
                    "index.html".to_string()
                } else {
                    format!("{path}/index.html")
                };

                let file = FRONTEND_ASSETS
                    .get_file(&route_index)
                    .or_else(|| FRONTEND_ASSETS.get_file(SPA_FALLBACK))
                    .unwrap();

                if let Some(contents_utf8) = file.contents_utf8() {
                    let content = inject_head(contents_utf8, Vec::new(), &state.version);

                    return ApiResponse::new(Body::from(content))
                        .with_header("Content-Type", "text/html")
                        .ok();
                }

                ApiResponse::new(Body::from(file.contents()))
                    .with_header("Content-Type", content_type_for(file.path().extension()))
                    .ok()
            })
            .layer(CatchPanicLayer::custom(handle_panic))
            .layer(RequestBodyTimeoutLayer::new(REQUEST_BODY_TIMEOUT))
            .layer(CorsLayer::permissive().allow_methods([Method::GET, Method::POST]))
            .layer(axum::middleware::from_fn(handle_request))
            .layer(CookieManagerLayer::new())
            .route_layer(axum::middleware::from_fn(handle_postprocessing))
            .route_layer(SentryHttpLayer::new().enable_transaction())
            .route_layer(NewSentryLayer::<Request>::new_from_top())
            .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", state.env.bind, state.env.port))
        .await
        .unwrap();

    tracing::info!(
        "http server listening on {} (app@{}, {}ms)",
        state.env.bind,
        VERSION,
        state.start_time.elapsed().as_millis()
    );

    let (router, mut openapi) = app.split_for_parts();
    openapi.info.version = state.version.clone();
    openapi.info.description = None;
    openapi.info.title = "MCJars API".to_string();
    openapi.info.contact = None;
    openapi.info.license = None;
    openapi.servers = Some(vec![utoipa::openapi::Server::new(
        state.env.app_url.clone(),
    )]);
    openapi.components.as_mut().unwrap().add_security_scheme(
        "api_key",
        SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("Authorization"))),
    );

    for (path, item) in openapi.paths.paths.iter_mut() {
        let operations = [
            ("get", &mut item.get),
            ("post", &mut item.post),
            ("put", &mut item.put),
            ("patch", &mut item.patch),
            ("delete", &mut item.delete),
        ];

        let path = path
            .replace('/', "_")
            .replace(|c| ['{', '}'].contains(&c), "");

        for (method, operation) in operations {
            if let Some(operation) = operation {
                operation.operation_id = Some(format!("{method}{path}"))
            }
        }
    }

    let openapi = Arc::new(openapi);
    let router = router.route("/openapi.json", get(|| async move { axum::Json(openapi) }));

    axum::serve(
        listener,
        ServiceExt::<Request>::into_make_service(
            NormalizePathLayer::trim_trailing_slash().layer(router),
        ),
    )
    .await
    .unwrap();
}
