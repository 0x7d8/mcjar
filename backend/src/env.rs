use anyhow::Context;
use dotenvy::dotenv;
use std::sync::Arc;
use tracing_subscriber::fmt::writer::MakeWriterExt;

#[derive(Clone)]
pub enum RedisMode {
    Redis,
    Sentinel,
}

#[derive(Clone)]
pub struct Env {
    pub redis_url: Option<String>,
    pub redis_sentinels: Option<Vec<String>>,
    pub redis_mode: RedisMode,

    pub sentry_url: Option<String>,
    pub database_migrate: bool,
    pub database_refresh: bool,
    pub database_url: String,
    pub database_url_primary: Option<String>,

    pub github_client_id: String,
    pub github_client_secret: String,

    pub backend_url: Option<String>,
    pub backend_refresh_token: Option<String>,

    pub s3_url: String,
    pub s3_path_style: bool,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,

    pub clickhouse_refresh: bool,
    pub clickhouse_url: String,
    pub clickhouse_database: String,
    pub clickhouse_username: String,
    pub clickhouse_password: String,

    pub files_cache: String,
    pub files_location: String,

    pub bind: String,
    pub port: u16,

    pub app_debug: bool,
    pub app_url: String,
    pub app_frontend_url: String,
    pub app_cookie_domain: String,
    pub app_log_directory: Option<String>,
    pub server_name: Option<String>,

    pub node_url: Option<String>,
    pub node_secret: Option<String>,
}

impl Env {
    pub fn parse() -> Result<
        (
            Arc<Self>,
            Option<tracing_appender::non_blocking::WorkerGuard>,
        ),
        anyhow::Error,
    > {
        dotenv().ok();

        let redis_mode = match std::env::var("REDIS_MODE")
            .unwrap_or("redis".to_string())
            .trim_matches('"')
        {
            "redis" => RedisMode::Redis,
            "sentinel" => RedisMode::Sentinel,
            _ => panic!("Invalid REDIS_MODE"),
        };

        let env = Self {
            redis_url: match redis_mode {
                RedisMode::Redis => Some(
                    std::env::var("REDIS_URL")
                        .expect("REDIS_URL is required")
                        .trim_matches('"')
                        .to_string(),
                ),
                RedisMode::Sentinel => None,
            },
            redis_sentinels: match redis_mode {
                RedisMode::Redis => None,
                RedisMode::Sentinel => Some(
                    std::env::var("REDIS_SENTINELS")
                        .expect("REDIS_SENTINELS is required")
                        .trim_matches('"')
                        .split(',')
                        .map(|s| s.to_string())
                        .collect(),
                ),
            },
            redis_mode,

            sentry_url: std::env::var("SENTRY_URL")
                .ok()
                .map(|s| s.trim_matches('"').to_string()),
            database_migrate: std::env::var("DATABASE_MIGRATE")
                .unwrap_or("false".to_string())
                .trim_matches('"')
                .parse()
                .unwrap(),
            database_refresh: std::env::var("DATABASE_REFRESH")
                .unwrap_or("false".to_string())
                .trim_matches('"')
                .parse()
                .unwrap(),
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL is required")
                .trim_matches('"')
                .to_string(),
            database_url_primary: std::env::var("DATABASE_URL_PRIMARY")
                .ok()
                .map(|s| s.trim_matches('"').to_string()),

            github_client_id: std::env::var("GITHUB_CLIENT_ID")
                .unwrap_or("".to_string())
                .trim_matches('"')
                .to_string(),
            github_client_secret: std::env::var("GITHUB_CLIENT_SECRET")
                .unwrap_or("".to_string())
                .trim_matches('"')
                .to_string(),

            backend_url: std::env::var("BACKEND_URL")
                .ok()
                .map(|s| s.trim_matches('"').to_string()),
            backend_refresh_token: std::env::var("BACKEND_REFRESH_TOKEN")
                .ok()
                .map(|s| s.trim_matches('"').to_string()),

            s3_url: std::env::var("S3_URL")
                .expect("S3_URL is required")
                .trim_matches('"')
                .to_string(),
            s3_path_style: std::env::var("S3_PATH_STYLE")
                .unwrap_or("true".to_string())
                .trim_matches('"')
                .parse()
                .unwrap(),
            s3_endpoint: std::env::var("S3_ENDPOINT")
                .expect("S3_ENDPOINT is required")
                .trim_matches('"')
                .to_string(),
            s3_region: std::env::var("S3_REGION")
                .expect("S3_REGION is required")
                .trim_matches('"')
                .to_string(),
            s3_bucket: std::env::var("S3_BUCKET")
                .expect("S3_BUCKET is required")
                .trim_matches('"')
                .to_string(),
            s3_access_key: std::env::var("S3_ACCESS_KEY")
                .expect("S3_ACCESS_KEY is required")
                .trim_matches('"')
                .to_string(),
            s3_secret_key: std::env::var("S3_SECRET_KEY")
                .expect("S3_SECRET_KEY is required")
                .trim_matches('"')
                .to_string(),

            clickhouse_refresh: std::env::var("CLICKHOUSE_REFRESH")
                .unwrap_or("false".to_string())
                .trim_matches('"')
                .parse()
                .unwrap(),
            clickhouse_url: std::env::var("CLICKHOUSE_URL")
                .expect("CLICKHOUSE_URL is required")
                .trim_matches('"')
                .to_string(),
            clickhouse_database: std::env::var("CLICKHOUSE_DATABASE")
                .expect("CLICKHOUSE_DATABASE is required")
                .trim_matches('"')
                .to_string(),
            clickhouse_username: std::env::var("CLICKHOUSE_USERNAME")
                .expect("CLICKHOUSE_USERNAME is required")
                .trim_matches('"')
                .to_string(),
            clickhouse_password: std::env::var("CLICKHOUSE_PASSWORD")
                .expect("CLICKHOUSE_PASSWORD is required")
                .trim_matches('"')
                .to_string(),

            bind: std::env::var("BIND")
                .unwrap_or("0.0.0.0".to_string())
                .trim_matches('"')
                .to_string(),
            port: std::env::var("PORT")
                .unwrap_or("6969".to_string())
                .parse()
                .unwrap(),

            files_cache: std::env::var("FILES_CACHE")
                .unwrap_or("/mnt/mcjars-cache".to_string())
                .trim_matches('"')
                .to_string(),
            files_location: std::env::var("FILES_LOCATION")
                .unwrap_or("/mnt/mcjars".to_string())
                .trim_matches('"')
                .to_string(),

            app_debug: std::env::var("APP_DEBUG")
                .unwrap_or("false".to_string())
                .trim_matches('"')
                .parse()
                .context("Invalid APP_DEBUG value")?,
            app_url: std::env::var("APP_URL")
                .expect("APP_URL is required")
                .trim_matches('"')
                .to_string(),
            app_frontend_url: std::env::var("APP_FRONTEND_URL")
                .expect("APP_FRONTEND_URL is required")
                .trim_matches('"')
                .trim_end_matches('/')
                .to_string(),
            app_cookie_domain: std::env::var("APP_COOKIE_DOMAIN")
                .expect("APP_COOKIE_DOMAIN is required")
                .trim_matches('"')
                .to_string(),
            app_log_directory: std::env::var("APP_LOG_DIRECTORY")
                .ok()
                .map(|s| s.trim_matches('"').to_string()),
            server_name: std::env::var("SERVER_NAME")
                .ok()
                .map(|s| s.trim_matches('"').to_string()),

            node_url: std::env::var("NODE_URL")
                .ok()
                .map(|s| s.trim_matches('"').trim_end_matches('/').to_string()),
            node_secret: std::env::var("NODE_SECRET")
                .ok()
                .map(|s| s.trim_matches('"').to_string()),
        };

        let (appender, guard) = if let Some(app_log_directory) = &env.app_log_directory {
            if !std::path::Path::new(app_log_directory).exists() {
                std::fs::create_dir_all(app_log_directory)
                    .context("failed to create log directory")?;
            }

            let latest_log_path = std::path::Path::new(&app_log_directory).join("panel.log");
            let latest_file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&latest_log_path)
                .context("failed to open latest log file")?;

            let rolling_appender = tracing_appender::rolling::Builder::new()
                .filename_prefix("panel")
                .filename_suffix("log")
                .max_log_files(30)
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .build(app_log_directory)
                .context("failed to create rolling log file appender")?;

            let (appender, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
                .buffered_lines_limit(50)
                .finish(latest_file.and(rolling_appender));

            (Some(appender), Some(guard))
        } else {
            (None, None)
        };
        if let Some(file_appender) = appender {
            tracing::subscriber::set_global_default(
                tracing_subscriber::fmt()
                    .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
                    .with_writer(std::io::stdout.and(file_appender))
                    .with_target(false)
                    .with_level(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_max_level(if env.app_debug {
                        tracing::Level::DEBUG
                    } else {
                        tracing::Level::INFO
                    })
                    .finish(),
            )?;
        } else {
            tracing::subscriber::set_global_default(
                tracing_subscriber::fmt()
                    .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
                    .with_target(false)
                    .with_level(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_max_level(if env.app_debug {
                        tracing::Level::DEBUG
                    } else {
                        tracing::Level::INFO
                    })
                    .finish(),
            )?;
        }

        Ok((Arc::new(env), guard))
    }
}
