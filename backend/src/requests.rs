use crate::{models::organization::Organization, routes::State};
use axum::{
    body::Bytes,
    http::{Method, request::Parts},
};
use chrono::NaiveDateTime;
use compact_str::ToCompactString;
use futures_util::Stream;
use rand::distr::SampleString;
use rustis::commands::ScriptingCommands;
use serde::{Deserialize, Serialize};
use sqlx::types::ipnetwork::IpNetwork;
use std::{
    collections::{HashMap, HashSet},
    net::Ipv6Addr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};
use tokio::sync::Mutex;

pub struct Request {
    id: String,
    organization_id: Option<i32>,
    end: bool,

    origin: String,
    method: Method,
    path: String,
    time: i32,
    status: i16,
    body: Option<serde_json::Value>,

    ip: IpNetwork,
    continent: Option<compact_str::CompactString>,
    country: Option<compact_str::CompactString>,

    data: Option<serde_json::Value>,
    user_agent: String,
    created: NaiveDateTime,
}

#[derive(Debug, Serialize, clickhouse::Row)]
pub struct ClickhouseRequest {
    id: [u8; 12],
    organization_id: Option<i32>,

    origin: Option<String>,
    method: i8,
    path: String,
    time: i32,
    status: i16,

    body: Option<String>,
    data: Option<String>,

    ip: Ipv6Addr,

    continent: Option<[u8; 2]>,
    country: Option<[u8; 2]>,

    user_agent: String,

    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    created: chrono::DateTime<chrono::Utc>,
}

impl From<Request> for ClickhouseRequest {
    fn from(req: Request) -> Self {
        Self {
            id: req.id.as_bytes().try_into().unwrap_or([0u8; 12]),
            organization_id: req.organization_id,
            origin: if req.origin.is_empty() {
                None
            } else {
                Some(req.origin)
            },
            method: match req.method {
                Method::GET => 1,
                Method::POST => 2,
                Method::PUT => 3,
                Method::DELETE => 4,
                Method::PATCH => 5,
                Method::OPTIONS => 6,
                Method::HEAD => 7,
                _ => 1,
            },
            path: req.path,
            time: req.time,
            status: req.status,
            body: req.body.map(|b| b.to_string()),
            data: req.data.map(|d| d.to_string()),
            ip: match req.ip {
                IpNetwork::V4(ipv4) => ipv4.ip().to_ipv6_mapped(),
                IpNetwork::V6(ipv6) => ipv6.ip(),
            },
            continent: req.continent.and_then(|c| c.as_bytes().try_into().ok()),
            country: req.country.and_then(|c| c.as_bytes().try_into().ok()),
            user_agent: req.user_agent,
            created: req.created.and_utc(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FileRequestKind {
    Index,
    File,
    Checksums,
}

impl FileRequestKind {
    #[inline]
    fn as_i8(self) -> i8 {
        match self {
            Self::Index => 1,
            Self::File => 2,
            Self::Checksums => 3,
        }
    }
}

pub struct FileRequest {
    id: String,
    organization_id: Option<i32>,

    origin: String,
    method: Method,
    path: String,
    root: String,
    kind: FileRequestKind,
    extension: String,
    size: i64,
    bytes_sent: i64,
    cache_hit: bool,
    time: i32,
    status: i16,

    ip: IpNetwork,
    continent: Option<compact_str::CompactString>,
    country: Option<compact_str::CompactString>,

    user_agent: String,
    created: NaiveDateTime,
}

#[derive(Debug, Serialize, clickhouse::Row)]
pub struct ClickhouseFileRequest {
    id: [u8; 12],
    organization_id: Option<i32>,

    origin: Option<String>,
    method: i8,
    path: String,
    root: String,
    kind: i8,
    extension: String,
    size: i64,
    bytes_sent: i64,
    cache_hit: bool,
    time: i32,
    status: i16,

    ip: Ipv6Addr,

    continent: Option<[u8; 2]>,
    country: Option<[u8; 2]>,

    user_agent: String,

    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    created: chrono::DateTime<chrono::Utc>,
}

impl From<FileRequest> for ClickhouseFileRequest {
    fn from(req: FileRequest) -> Self {
        Self {
            id: req.id.as_bytes().try_into().unwrap_or([0u8; 12]),
            organization_id: req.organization_id,
            origin: if req.origin.is_empty() {
                None
            } else {
                Some(req.origin)
            },
            method: match req.method {
                Method::GET => 1,
                Method::POST => 2,
                Method::PUT => 3,
                Method::DELETE => 4,
                Method::PATCH => 5,
                Method::OPTIONS => 6,
                Method::HEAD => 7,
                _ => 1,
            },
            path: req.path,
            root: req.root,
            kind: req.kind.as_i8(),
            extension: req.extension,
            size: req.size,
            bytes_sent: req.bytes_sent,
            cache_hit: req.cache_hit,
            time: req.time,
            status: req.status,
            ip: match req.ip {
                IpNetwork::V4(ipv4) => ipv4.ip().to_ipv6_mapped(),
                IpNetwork::V6(ipv6) => ipv6.ip(),
            },
            continent: req.continent.and_then(|c| c.as_bytes().try_into().ok()),
            country: req.country.and_then(|c| c.as_bytes().try_into().ok()),
            user_agent: req.user_agent,
            created: req.created.and_utc(),
        }
    }
}

const ACCEPTED_METHODS: &[Method] = &[
    Method::GET,
    Method::POST,
    Method::PUT,
    Method::DELETE,
    Method::PATCH,
];

#[derive(Debug, Clone, Copy)]
pub struct RateLimitData {
    pub limit: i64,
    pub hits: i64,
    pub reset: i64,
}

#[derive(Debug, Clone, Copy)]
pub enum RateLimitError {
    Requests(RateLimitData),
    Bandwidth { limit: i64, used: i64, reset: i64 },
}

#[derive(Debug, Clone, Copy)]
enum RateLimitBucket {
    Regular,
    FilesBrowse,
    FilesDownload,
}

impl RateLimitBucket {
    fn from_request(path: &str, method: &Method) -> Self {
        if path != "/files" && !path.starts_with("/files/") {
            return Self::Regular;
        }

        if method != Method::HEAD && (path.ends_with(".jar") || path.ends_with(".zip")) {
            Self::FilesDownload
        } else {
            Self::FilesBrowse
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::FilesBrowse => "files",
            Self::FilesDownload => "files_download",
        }
    }

    fn limit(self, organization: Option<&Organization>) -> i64 {
        let base = match self {
            Self::Regular | Self::FilesBrowse => 120,
            Self::FilesDownload => 30,
        };

        if organization.is_some() {
            base * 2
        } else {
            base
        }
    }
}

const RATELIMIT_WINDOW: i64 = 60;
const RATELIMIT_SCRIPT: &str = r#"
local hits = redis.call('INCR', KEYS[1])
local ttl = redis.call('TTL', KEYS[1])
if ttl < 0 then
  redis.call('EXPIRE', KEYS[1], ARGV[1])
  ttl = tonumber(ARGV[1])
end
return {hits, ttl}
"#;

const BANDWIDTH_WINDOW: i64 = 60 * 60 * 24;
const BANDWIDTH_LIMIT: i64 = 25 * 1024 * 1024 * 1024;
const BANDWIDTH_LIMIT_VERIFIED: i64 = 100 * 1024 * 1024 * 1024;
const BANDWIDTH_SCRIPT: &str = r#"
local used = redis.call('INCRBY', KEYS[1], ARGV[1])
local ttl = redis.call('TTL', KEYS[1])
if ttl < 0 then
  redis.call('EXPIRE', KEYS[1], ARGV[2])
  ttl = tonumber(ARGV[2])
end
return {used, ttl}
"#;

#[inline]
fn bandwidth_key(ip: std::net::IpAddr, organization_id: Option<i32>) -> String {
    match organization_id {
        Some(id) => format!("mcjars_api::bandwidth::organization::{id}"),
        None => format!("mcjars_api::bandwidth::{ip}"),
    }
}

#[inline]
fn bandwidth_limit(organization: Option<&Organization>) -> i64 {
    if organization.is_some_and(|o| o.verified) {
        BANDWIDTH_LIMIT_VERIFIED
    } else {
        BANDWIDTH_LIMIT
    }
}

pub struct RequestLogger {
    pending: Mutex<Vec<Request>>,
    processing: Mutex<Vec<Request>>,
    pending_files: Mutex<Vec<FileRequest>>,
    processing_files: Mutex<Vec<FileRequest>>,
    uncounted_requests: Mutex<i64>,
    database: Arc<crate::database::Database>,
    clickhouse: Arc<crate::clickhouse::Clickhouse>,
    cache: Arc<crate::cache::Cache>,

    client: reqwest::Client,
}

impl RequestLogger {
    pub fn new(
        database: Arc<crate::database::Database>,
        clickhouse: Arc<crate::clickhouse::Clickhouse>,
        cache: Arc<crate::cache::Cache>,
    ) -> Self {
        Self {
            pending: Mutex::new(Vec::new()),
            processing: Mutex::new(Vec::new()),
            pending_files: Mutex::new(Vec::new()),
            processing_files: Mutex::new(Vec::new()),
            uncounted_requests: Mutex::new(0),
            database,
            clickhouse,
            cache,

            client: reqwest::Client::builder()
                .user_agent("MCJars API https://mcjars.app")
                .build()
                .unwrap(),
        }
    }

    pub async fn log(
        &self,
        request: &Parts,
        organization: Option<&Organization>,
    ) -> Result<(Option<String>, Option<RateLimitData>), RateLimitError> {
        let ip = match crate::utils::extract_ip(&request.headers) {
            Some(ip) => ip,
            None => std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        };

        let bucket = RateLimitBucket::from_request(request.uri.path(), &request.method);

        let mut ratelimit: Option<RateLimitData> = None;
        if organization.is_none_or(|o| !o.verified) {
            let ratelimit_key = format!("mcjars_api::ratelimit::{ip}::{}", bucket.suffix());

            let (hits, reset): (i64, i64) = self
                .cache
                .client
                .eval(
                    RATELIMIT_SCRIPT,
                    [ratelimit_key.as_str()],
                    [RATELIMIT_WINDOW],
                )
                .await
                .unwrap_or((0, RATELIMIT_WINDOW));

            let data = RateLimitData {
                limit: bucket.limit(organization),
                hits,
                reset,
            };
            ratelimit = Some(data);

            if hits > data.limit {
                return Err(RateLimitError::Requests(data));
            }
        }

        if matches!(bucket, RateLimitBucket::FilesDownload) {
            let key = bandwidth_key(ip, organization.map(|o| o.id));
            let limit = bandwidth_limit(organization);

            let (used, reset): (i64, i64) = self
                .cache
                .client
                .eval(BANDWIDTH_SCRIPT, [key.as_str()], [0, BANDWIDTH_WINDOW])
                .await
                .unwrap_or((0, BANDWIDTH_WINDOW));

            if used >= limit {
                return Err(RateLimitError::Bandwidth { limit, used, reset });
            }
        }

        *self.uncounted_requests.lock().await += 1;

        if ACCEPTED_METHODS.iter().all(|m| *m != request.method)
            || !request.uri.path().starts_with("/api")
            || request.uri.path().starts_with("/api/github")
        {
            return Ok((None, ratelimit));
        };

        let data = Request {
            id: rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 12),
            organization_id: organization.map(|o| o.id),
            end: false,

            origin: request
                .headers
                .get("origin")
                .map(|o| crate::utils::slice_up_to(o.to_str().unwrap_or("unknown"), 255))
                .unwrap_or("")
                .to_string(),
            method: request.method.clone(),
            path: crate::utils::slice_up_to(
                &format!(
                    "{}{}",
                    request.uri.path(),
                    request
                        .uri
                        .query()
                        .map(|q| format!("?{}", q.replacen("tracking=none", "tracking=nostats", 1)))
                        .unwrap_or_default()
                ),
                255,
            )
            .to_string(),
            time: 0,
            status: 0,
            body: None,

            ip: ip.into(),
            continent: None,
            country: None,

            data: None,
            user_agent: request
                .headers
                .get("User-Agent")
                .map(|ua| crate::utils::slice_up_to(ua.to_str().unwrap_or("unknown"), 255))
                .unwrap_or("unknown")
                .to_string(),
            created: chrono::Utc::now().naive_utc(),
        };

        let id = data.id.clone();
        self.pending.lock().await.push(data);

        Ok((Some(id), ratelimit))
    }

    pub async fn finish(
        &self,
        id: String,
        status: i16,
        time: i32,
        data: Option<serde_json::Value>,
        body: Option<serde_json::Value>,
    ) {
        let mut pending = self.pending.lock().await;

        if let Some(index) = pending.iter().position(|r| r.id == id) {
            let mut request = pending.remove(index);

            request.end = true;
            request.status = status;
            request.time = time;
            request.data = data;
            request.body = body;

            self.processing.lock().await.push(request);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn log_file(
        &self,
        request: &Parts,
        organization: Option<&Organization>,
        kind: FileRequestKind,
        path: &std::path::Path,
        size: i64,
        cache_hit: bool,
    ) -> String {
        let ip = match crate::utils::extract_ip(&request.headers) {
            Some(ip) => ip,
            None => std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        };

        let path_display = path.to_string_lossy();
        let root = path
            .components()
            .find_map(|c| c.as_os_str().to_str().filter(|s| !s.is_empty()))
            .unwrap_or("")
            .to_string();
        // Path::extension reports "4" for a directory like `vanilla/1.21.4`.
        let extension = match kind {
            FileRequestKind::Index => String::new(),
            _ => path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string(),
        };

        let data = FileRequest {
            id: rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 12),
            organization_id: organization.map(|o| o.id),

            origin: request
                .headers
                .get("origin")
                .map(|o| crate::utils::slice_up_to(o.to_str().unwrap_or("unknown"), 255))
                .unwrap_or("")
                .to_string(),
            method: request.method.clone(),
            path: crate::utils::slice_up_to(&path_display, 255).to_string(),
            root: crate::utils::slice_up_to(&root, 255).to_string(),
            kind,
            extension: crate::utils::slice_up_to(&extension, 32).to_string(),
            size,
            bytes_sent: 0,
            cache_hit,
            time: 0,
            status: 0,

            ip: ip.into(),
            continent: None,
            country: None,

            user_agent: request
                .headers
                .get("User-Agent")
                .map(|ua| crate::utils::slice_up_to(ua.to_str().unwrap_or("unknown"), 255))
                .unwrap_or("unknown")
                .to_string(),
            created: chrono::Utc::now().naive_utc(),
        };

        let id = data.id.clone();
        self.pending_files.lock().await.push(data);

        id
    }

    pub async fn finish_file(&self, id: String, status: i16, time: i32, bytes_sent: i64) {
        let mut pending = self.pending_files.lock().await;

        let Some(index) = pending.iter().position(|r| r.id == id) else {
            return;
        };

        let mut request = pending.remove(index);
        drop(pending);

        request.status = status;
        request.time = time;
        request.bytes_sent = bytes_sent;

        if bytes_sent > 0 {
            let key = bandwidth_key(request.ip.ip(), request.organization_id);

            let _: (i64, i64) = self
                .cache
                .client
                .eval(
                    BANDWIDTH_SCRIPT,
                    [key.as_str()],
                    [bytes_sent, BANDWIDTH_WINDOW],
                )
                .await
                .unwrap_or((0, BANDWIDTH_WINDOW));
        }

        self.processing_files.lock().await.push(request);
    }

    #[inline]
    async fn lookup_ips(
        &self,
        ips: Vec<compact_str::CompactString>,
    ) -> Result<HashMap<compact_str::CompactString, [compact_str::CompactString; 2]>, reqwest::Error>
    {
        let mut result = HashMap::new();

        let data = self
            .client
            .post("http://ip-api.com/batch")
            .header("Content-Type", "application/json")
            .json(
                &ips.into_iter()
                    .map(|ip| {
                        serde_json::json!({
                            "query": ip,
                            "fields": "continentCode,countryCode,query"
                        })
                    })
                    .collect::<HashSet<_>>(),
            )
            .send()
            .await?
            .json::<Vec<IpApiResponse>>()
            .await?;

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct IpApiResponse {
            continent_code: compact_str::CompactString,
            country_code: compact_str::CompactString,
            query: compact_str::CompactString,
        }

        for entry in data {
            result.insert(entry.query, [entry.continent_code, entry.country_code]);
        }

        Ok(result)
    }

    pub async fn process(&self) -> Result<(), anyhow::Error> {
        let now = chrono::Utc::now().naive_utc();

        let mut processing = self.processing.lock().await;
        let length = processing.len();

        self.pending
            .lock()
            .await
            .retain(|r| r.created > now - chrono::Duration::seconds(60));

        let mut requests = processing
            .splice(0..std::cmp::min(30, length), Vec::new())
            .collect::<Vec<_>>();
        processing.retain(|r| r.created > now - chrono::Duration::seconds(300));

        drop(processing);

        let mut processing_files = self.processing_files.lock().await;
        let files_length = processing_files.len();

        self.pending_files
            .lock()
            .await
            .retain(|r| r.created > now - chrono::Duration::seconds(3600));

        let mut file_requests = processing_files
            .splice(0..std::cmp::min(30, files_length), Vec::new())
            .collect::<Vec<_>>();
        processing_files.retain(|r| r.created > now - chrono::Duration::seconds(3600));

        drop(processing_files);

        if requests.is_empty() && file_requests.is_empty() {
            return Ok(());
        }

        let ips = self
            .lookup_ips(
                requests
                    .iter()
                    .map(|t| t.ip.to_compact_string())
                    .chain(file_requests.iter().map(|t| t.ip.to_compact_string()))
                    .collect::<Vec<_>>(),
            )
            .await
            .unwrap_or_default();

        for r in requests.iter_mut() {
            if let Some([continent, country]) = ips.get(&r.ip.to_compact_string()) {
                r.continent = Some(continent.clone());
                r.country = Some(country.clone());
            }
        }

        for r in file_requests.iter_mut() {
            if let Some([continent, country]) = ips.get(&r.ip.to_compact_string()) {
                r.continent = Some(continent.clone());
                r.country = Some(country.clone());
            }
        }

        let requests_len = requests.len();
        let file_requests_len = file_requests.len();

        if !requests.is_empty() {
            let mut insert = self
                .clickhouse
                .client()
                .insert::<ClickhouseRequest>("requests")
                .await?;
            for r in requests {
                let ch_request: ClickhouseRequest = r.into();

                insert.write(&ch_request).await?;
            }
            insert.end().await?;
        }

        if !file_requests.is_empty() {
            let mut insert = self
                .clickhouse
                .client()
                .insert::<ClickhouseFileRequest>("file_requests")
                .await?;
            for r in file_requests {
                let ch_request: ClickhouseFileRequest = r.into();

                insert.write(&ch_request).await?;
            }
            insert.end().await?;
        }

        let mut uncounted_requests = self.uncounted_requests.lock().await;
        if *uncounted_requests > 0 {
            let count = *uncounted_requests;
            *uncounted_requests = 0;
            drop(uncounted_requests);

            if let Err(err) = self.database.update_count("requests", count).await {
                tracing::error!("failed to update request count: {:?}", err);
            }
        }

        tracing::info!(
            "processed {} requests, {} file requests",
            requests_len,
            file_requests_len
        );

        Ok(())
    }
}

/// Counts the bytes of a file download that actually reach the client and finalises
/// the request on drop, so an aborted download is distinguishable from a completed one.
pub struct TrackedFileStream<S> {
    inner: S,
    state: State,
    id: Option<String>,
    status: i16,
    bytes_sent: u64,
    started: Instant,
}

impl<S> TrackedFileStream<S> {
    pub fn new(inner: S, state: State, id: String, status: i16, started: Instant) -> Self {
        Self {
            inner,
            state,
            id: Some(id),
            status,
            bytes_sent: 0,
            started,
        }
    }
}

impl<S, E> Stream for TrackedFileStream<S>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
{
    type Item = Result<Bytes, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let polled = Pin::new(&mut self.inner).poll_next(cx);

        if let Poll::Ready(Some(Ok(chunk))) = &polled {
            self.bytes_sent += chunk.len() as u64;
        }

        polled
    }
}

impl<S> Drop for TrackedFileStream<S> {
    fn drop(&mut self) {
        let Some(id) = self.id.take() else {
            return;
        };

        let state = self.state.clone();
        let status = self.status;
        let bytes_sent = self.bytes_sent as i64;
        let time = self.started.elapsed().as_millis() as i32;

        tokio::spawn(async move {
            state
                .requests
                .finish_file(id, status, time, bytes_sent)
                .await;
        });
    }
}
