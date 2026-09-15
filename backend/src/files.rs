use crate::models::file::File;
use chrono::NaiveDateTime;
use compact_str::ToCompactString;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Mutex, RwLock, watch},
};

const BUFFER_SIZE: usize = 64 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FillStatus {
    InProgress,
    Done,
    Failed,
}

#[derive(Clone, Copy)]
struct FillProgress {
    written: u64,
    status: FillStatus,
}

struct CachedFile {
    id: u64,
    size: u64,
    ready: bool,

    last_access: NaiveDateTime,
    last_access_written: bool,

    progress: watch::Receiver<FillProgress>,
}

type CachedFiles = HashMap<Arc<PathBuf>, Arc<Mutex<CachedFile>>>;

pub struct FileCache {
    id: Arc<AtomicU64>,
    total_size: Arc<AtomicU64>,
    max_cache_size: u64,
    cached_files: Arc<RwLock<CachedFiles>>,

    database: Arc<crate::database::Database>,
    env: Arc<crate::env::Env>,
}

impl FileCache {
    pub async fn new(database: Arc<crate::database::Database>, env: Arc<crate::env::Env>) -> Self {
        tokio::fs::remove_dir_all(&env.files_cache)
            .await
            .unwrap_or_default();
        tokio::fs::create_dir_all(&env.files_cache).await.unwrap();

        Self {
            id: Arc::new(AtomicU64::new(0)),
            total_size: Arc::new(AtomicU64::new(0)),
            max_cache_size: 5 * 1024 * 1024 * 1024,
            cached_files: Arc::new(RwLock::new(HashMap::new())),
            database,
            env,
        }
    }

    #[inline]
    pub fn max_cache_size(&self) -> u64 {
        self.max_cache_size
    }

    #[inline]
    pub fn cache_size(&self) -> u64 {
        self.total_size.load(Ordering::Relaxed)
    }

    pub async fn cached_file_count(&self) -> usize {
        self.cached_files.read().await.len()
    }

    /// Returns whether the file was already resident in the local cache alongside its reader.
    pub async fn get(
        &self,
        path: &Path,
        file: &File,
    ) -> std::io::Result<(bool, Box<dyn tokio::io::AsyncRead + Send + Unpin>)> {
        if file.is_directory {
            return Err(std::io::Error::other("cannot get file for directory"));
        }

        let key = Arc::new(path.to_path_buf());

        let existing = self.cached_files.read().await.get(&key).cloned();
        if let Some(entry) = existing {
            return Ok((true, self.serve(entry).await?));
        }

        let file_size = file.size as u64;
        let mut map = self.cached_files.write().await;

        if let Some(entry) = map.get(&key).cloned() {
            drop(map);
            return Ok((true, self.serve(entry).await?));
        }

        if self.total_size.load(Ordering::Relaxed) + file_size > self.max_cache_size {
            self.make_space_for_file(file_size, &mut map).await?;
        }

        let id = self.id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = watch::channel(FillProgress {
            written: 0,
            status: FillStatus::InProgress,
        });

        let entry = Arc::new(Mutex::new(CachedFile {
            id,
            size: file_size,
            ready: false,
            last_access: chrono::Utc::now().naive_utc(),
            last_access_written: false,
            progress: rx,
        }));

        map.insert(key.clone(), entry.clone());
        self.total_size.fetch_add(file_size, Ordering::Relaxed);
        drop(map);

        self.spawn_fill(key, entry.clone(), path.to_path_buf(), id, file_size, tx);

        Ok((false, self.serve(entry).await?))
    }

    async fn serve(
        &self,
        entry: Arc<Mutex<CachedFile>>,
    ) -> std::io::Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>> {
        let (id, ready, progress) = {
            let mut entry = entry.lock().await;
            entry.last_access = chrono::Utc::now().naive_utc();
            entry.last_access_written = false;
            (entry.id, entry.ready, entry.progress.clone())
        };

        let path = Path::new(&self.env.files_cache).join(id.to_string());

        if ready {
            return Ok(Box::new(tokio::fs::File::open(path).await?));
        }

        Ok(Box::new(Self::follow(path, progress)))
    }

    fn follow(
        path: PathBuf,
        mut progress: watch::Receiver<FillProgress>,
    ) -> impl tokio::io::AsyncRead + Send + Unpin {
        let (reader, mut writer) = tokio::io::duplex(BUFFER_SIZE);

        tokio::spawn(async move {
            let mut run = async || -> std::io::Result<()> {
                let mut file = loop {
                    let current_progress = *progress.borrow_and_update();

                    match current_progress.status {
                        FillStatus::Failed => {
                            return Err(std::io::Error::other("cache fill failed"));
                        }
                        _ if current_progress.written > 0
                            || current_progress.status == FillStatus::Done =>
                        {
                            break tokio::fs::File::open(&path).await?;
                        }
                        _ => {
                            if progress.changed().await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                };

                let mut buffer = vec![0; BUFFER_SIZE];
                let mut sent = 0;

                loop {
                    let current_progress = *progress.borrow_and_update();

                    while sent < current_progress.written {
                        match file.read(&mut buffer).await? {
                            0 => break,
                            n => {
                                writer.write_all(&buffer[..n]).await?;
                                sent += n as u64;
                            }
                        }
                    }

                    match current_progress.status {
                        FillStatus::Done if sent >= current_progress.written => return Ok(()),
                        FillStatus::Failed => {
                            return Err(std::io::Error::other("cache fill failed"));
                        }
                        _ => {
                            if progress.changed().await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                }
            };

            if let Err(err) = run().await {
                tracing::warn!("cache follower stopped early: {err:?}");
            }
        });

        reader
    }

    fn spawn_fill(
        &self,
        key: Arc<PathBuf>,
        entry: Arc<Mutex<CachedFile>>,
        rel_path: PathBuf,
        id: u64,
        reserved: u64,
        tx: watch::Sender<FillProgress>,
    ) {
        let source = Path::new(&self.env.files_location).join(&rel_path);
        let destination = Path::new(&self.env.files_cache).join(id.to_string());
        let cached_files = self.cached_files.clone();
        let total_size = self.total_size.clone();

        tokio::spawn(async move {
            let mut written = 0;

            let run = async {
                let mut reader = tokio::fs::File::open(&source).await?;
                let mut cache = tokio::fs::File::create(&destination).await?;
                let mut buffer = vec![0; BUFFER_SIZE];

                loop {
                    match reader.read(&mut buffer).await? {
                        0 => break,
                        n => {
                            cache.write_all(&buffer[..n]).await?;
                            written += n as u64;

                            tx.send_replace(FillProgress {
                                written,
                                status: FillStatus::InProgress,
                            });
                        }
                    }
                }

                cache.flush().await?;
                Ok::<_, std::io::Error>(())
            }
            .await;

            match run {
                Ok(()) => {
                    tx.send_replace(FillProgress {
                        written,
                        status: FillStatus::Done,
                    });

                    {
                        let mut entry = entry.lock().await;
                        entry.size = written;
                        entry.ready = true;
                    }

                    if written >= reserved {
                        total_size.fetch_add(written - reserved, Ordering::Relaxed);
                    } else {
                        total_size.fetch_sub(reserved - written, Ordering::Relaxed);
                    }
                }
                Err(err) => {
                    tx.send_replace(FillProgress {
                        written: 0,
                        status: FillStatus::Failed,
                    });

                    cached_files.write().await.remove(&key);
                    tokio::fs::remove_file(&destination)
                        .await
                        .unwrap_or_default();
                    total_size.fetch_sub(reserved, Ordering::Relaxed);

                    tracing::error!("cache fill failed for {}: {err:?}", source.display());
                }
            }
        });
    }

    async fn make_space_for_file(
        &self,
        required_size: u64,
        cached_files: &mut CachedFiles,
    ) -> std::io::Result<()> {
        if required_size > self.max_cache_size {
            return Err(std::io::Error::other(format!(
                "file size {} exceeds maximum cache size {}",
                required_size, self.max_cache_size
            )));
        }

        let mut candidates = cached_files
            .iter()
            .filter_map(|(path, file)| {
                let file = file.try_lock().ok()?;
                if !file.ready {
                    return None;
                }
                Some((path.clone(), file.last_access, file.id, file.size))
            })
            .collect::<Vec<_>>();

        candidates.sort_by_key(|a| a.1);

        let current_size = self.total_size.load(Ordering::Relaxed);
        let target_size = (current_size + required_size).saturating_sub(self.max_cache_size);

        tracing::info!(
            "cache size: {}/{} bytes, need to free {} bytes",
            current_size,
            self.max_cache_size,
            target_size
        );

        let mut freed_size = 0;
        let mut removed_count = 0;

        for (path, _, id, size) in candidates {
            if freed_size >= target_size {
                break;
            }

            let removed =
                match tokio::fs::remove_file(Path::new(&self.env.files_cache).join(id.to_string()))
                    .await
                {
                    Ok(_) => true,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
                    Err(err) => {
                        tracing::error!(
                            "failed to remove file {} from cache: {:?}",
                            path.display(),
                            err
                        );
                        false
                    }
                };

            if removed {
                freed_size += size;
                removed_count += 1;
                cached_files.remove(&path);
                self.total_size.fetch_sub(size, Ordering::Relaxed);
            }
        }

        tracing::info!(
            "freed {} bytes by removing {} files from cache",
            freed_size,
            removed_count
        );

        if freed_size < target_size {
            return Err(std::io::Error::other(format!(
                "could not free enough space in cache. needed {} bytes, freed {} bytes",
                target_size, freed_size
            )));
        }

        Ok(())
    }

    pub async fn process(&self) -> Result<(), anyhow::Error> {
        let pending_files = {
            let cached_files = self.cached_files.read().await;
            cached_files
                .iter()
                .filter_map(|(path, file)| {
                    let file = file.try_lock().ok()?;
                    if file.last_access_written {
                        return None;
                    }
                    Some((path.clone(), file.last_access))
                })
                .collect::<Vec<_>>()
        };

        let pending_files_len = pending_files.len();

        for (path, last_access) in pending_files {
            if let Some(entry) = self.cached_files.read().await.get(&path) {
                entry.lock().await.last_access_written = true;
            }

            if let Err(err) = sqlx::query!(
                r#"
                UPDATE files
                SET last_access = $1
                WHERE files.path = $2::varchar[] AND (files.last_access IS NULL OR files.last_access < $1)
                "#,
                last_access,
                &path
                    .components()
                    .filter(|c| c.as_os_str().to_str().is_some_and(|s| !s.is_empty()))
                    .map(|c| c.as_os_str().to_string_lossy().to_compact_string())
                    .collect::<Vec<_>>() as &[compact_str::CompactString]
            )
            .execute(self.database.write())
            .await
            {
                tracing::error!("failed to update file {}: {:?}", path.display(), err);
            }
        }

        if pending_files_len > 0 {
            tracing::info!("processed {} pending files", pending_files_len);
        }

        let deletion_threshold =
            chrono::Utc::now().naive_utc() - std::time::Duration::from_hours(24);

        let mut cached_files = self.cached_files.write().await;
        let deletable = cached_files
            .iter()
            .filter_map(|(path, file)| {
                let file = file.try_lock().ok()?;
                if !file.ready || file.last_access >= deletion_threshold {
                    return None;
                }
                Some((path.clone(), file.id, file.size))
            })
            .collect::<Vec<_>>();

        for (path, id, size) in deletable {
            match tokio::fs::remove_file(Path::new(&self.env.files_cache).join(id.to_string()))
                .await
            {
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    tracing::error!("failed to evict file {}: {:?}", path.display(), err);
                    continue;
                }
            }

            cached_files.remove(&path);
            self.total_size.fetch_sub(size, Ordering::Relaxed);
        }

        Ok(())
    }
}
