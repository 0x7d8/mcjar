use crate::models::node::Node;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{sync::Arc, time::Instant};
use utoipa::ToSchema;

const PEER_TIMEOUT_SECONDS: u64 = 5;

#[derive(ToSchema, Serialize)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct NodeResult<T: Serialize> {
    pub node: compact_str::CompactString,
    pub url: compact_str::CompactString,
    pub version: compact_str::CompactString,

    pub local: bool,
    pub latency: Option<i64>,

    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(ToSchema, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemSnapshot {
    pub version: String,
    pub uptime: u64,

    pub idle_read_connections: usize,
    pub idle_write_connections: usize,

    pub cache_hits: usize,
    pub cache_misses: usize,
    pub cache_memory: Option<u64>,

    pub file_cache_files: usize,
    pub file_cache_size: u64,
    pub file_cache_max_size: u64,

    pub pending_requests: usize,
    pub pending_file_requests: usize,
}

impl SystemSnapshot {
    pub async fn capture(state: &crate::routes::State) -> Result<Self, anyhow::Error> {
        let (pending_requests, pending_file_requests) = state.requests.queue_depth().await;

        Ok(Self {
            version: state.version.clone(),
            uptime: state.start_time.elapsed().as_secs(),

            idle_read_connections: state.database.read().num_idle(),
            idle_write_connections: state.database.write().num_idle(),

            cache_hits: state.cache.cache_hits(),
            cache_misses: state.cache.cache_misses(),
            cache_memory: state.cache.used_memory().await,

            file_cache_files: state.files.cached_file_count().await,
            file_cache_size: state.files.cache_size(),
            file_cache_max_size: state.files.max_cache_size(),

            pending_requests,
            pending_file_requests,
        })
    }
}

pub struct NodeClient {
    client: reqwest::Client,

    database: Arc<crate::database::Database>,
    env: Arc<crate::env::Env>,
}

impl NodeClient {
    pub fn new(database: Arc<crate::database::Database>, env: Arc<crate::env::Env>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("MCJars API https://mcjars.app")
                .timeout(std::time::Duration::from_secs(PEER_TIMEOUT_SECONDS))
                .build()
                .unwrap(),

            database,
            env,
        }
    }

    #[inline]
    pub fn name(&self) -> Option<&str> {
        self.env.server_name.as_deref()
    }

    pub async fn heartbeat(&self, version: &str) -> Result<(), anyhow::Error> {
        let (Some(name), Some(url)) = (self.name(), self.env.node_url.as_deref()) else {
            return Ok(());
        };

        Node::heartbeat(&self.database, name, url, version).await
    }

    pub async fn all(&self) -> Result<Vec<Node>, anyhow::Error> {
        Node::all(&self.database).await
    }

    pub async fn peers(&self) -> Result<Vec<Node>, anyhow::Error> {
        let name = self.name();

        Ok(Node::all(&self.database)
            .await?
            .into_iter()
            .filter(|node| node.alive() && Some(node.name.as_str()) != name)
            .collect())
    }

    pub async fn fanout<T: DeserializeOwned + Serialize>(
        &self,
        path: &str,
    ) -> Result<Vec<NodeResult<T>>, anyhow::Error> {
        let Some(secret) = self.env.node_secret.as_deref() else {
            return Ok(Vec::new());
        };

        let peers = self.peers().await?;

        Ok(
            futures_util::future::join_all(peers.into_iter().map(|node| async move {
                let start = Instant::now();
                let result = self.request::<T>(&node.url, path, secret).await;
                let latency = start.elapsed().as_millis() as i64;

                match result {
                    Ok(data) => NodeResult {
                        node: node.name,
                        url: node.url,
                        version: node.version,
                        local: false,
                        latency: Some(latency),
                        data: Some(data),
                        error: None,
                    },
                    Err(err) => NodeResult {
                        node: node.name,
                        url: node.url,
                        version: node.version,
                        local: false,
                        latency: Some(latency),
                        data: None,
                        error: Some(err.to_string()),
                    },
                }
            }))
            .await,
        )
    }

    async fn request<T: DeserializeOwned>(
        &self,
        url: &str,
        path: &str,
        secret: &str,
    ) -> Result<T, anyhow::Error> {
        let response = self
            .client
            .get(format!("{url}/api/internal{path}"))
            .bearer_auth(secret)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("peer responded with {status}"));
        }

        Ok(response.json().await?)
    }
}
