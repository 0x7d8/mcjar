use super::{GetState, State};
use crate::{
    models::{file::File, organization::Organization},
    requests::{FileRequestKind, TrackedFileStream},
    response::ApiResponse,
    routes::index::{IndexFile, render, render_not_found, render_robots},
};
use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode, request::Parts},
    routing::{any, get},
};
use std::{
    path::{Component, Path, PathBuf},
    time::Instant,
};
use utoipa_axum::router::OpenApiRouter;

/// md5, sha1, sha224, sha256, sha384 and sha512: 416 hex chars, 42 chars of
/// padded label, 6 newlines.
const CHECKSUMS_LENGTH: i64 = 464;

struct CompletedFile<'a> {
    kind: FileRequestKind,
    path: &'a Path,
    size: i64,
    bytes_sent: i64,
    status: StatusCode,
    started: Instant,
}

async fn log_complete(
    state: &GetState,
    parts: &Parts,
    organization: Option<&Organization>,
    completed: CompletedFile<'_>,
) {
    let id = state
        .requests
        .log_file(
            parts,
            organization,
            completed.kind,
            completed.path,
            completed.size,
            false,
        )
        .await;

    state
        .requests
        .finish_file(
            id,
            completed.status.as_u16() as i16,
            completed.started.elapsed().as_millis() as i32,
            completed.bytes_sent,
        )
        .await;
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .route("/robots.txt", get(|| async { render_robots() }))
        .fallback(any(|state: GetState, request: Request| async move {
            let started = Instant::now();
            let (parts, _) = request.into_parts();
            let organization = parts
                .extensions
                .get::<Option<Organization>>()
                .cloned()
                .flatten();
            let organization = organization.as_ref();

            let path = Path::new(parts.uri.path().strip_prefix('/').unwrap_or_default());

            if path.components().any(|c| matches!(c, Component::ParentDir)) {
                return render_not_found(&state, &format!("/{}", path.to_string_lossy()));
            }

            if path.components().next_back().is_some_and(|c| {
                let string = c.as_os_str().to_string_lossy();

                string.ends_with(".txt") || string.ends_with(".jar") || string.ends_with(".zip")
            }) {
                let last = path
                    .components()
                    .next_back()
                    .unwrap()
                    .as_os_str()
                    .to_string_lossy();

                if let Some(last) = last.strip_suffix(".CHECKSUMS.txt") {
                    let file = match File::by_path(
                        &state.database,
                        &state.cache,
                        &path
                            .components()
                            .take(path.components().count() - 1)
                            .collect::<PathBuf>()
                            .join(last),
                    )
                    .await?
                    {
                        Some(file) => file,
                        None => {
                            log_complete(
                                &state,
                                &parts,
                                organization,
                                CompletedFile {
                                    kind: FileRequestKind::Checksums,
                                    path,
                                    size: 0,
                                    bytes_sent: 0,
                                    status: StatusCode::NOT_FOUND,
                                    started,
                                },
                            )
                            .await;

                            return render_not_found(
                                &state,
                                &format!("/{}", path.to_string_lossy()),
                            );
                        }
                    };

                    log_complete(
                        &state,
                        &parts,
                        organization,
                        CompletedFile {
                            kind: FileRequestKind::Checksums,
                            path,
                            size: CHECKSUMS_LENGTH,
                            bytes_sent: if parts.method == Method::HEAD {
                                0
                            } else {
                                CHECKSUMS_LENGTH
                            },
                            status: StatusCode::OK,
                            started,
                        },
                    )
                    .await;

                    if parts.method == Method::HEAD {
                        return ApiResponse::new(Body::empty())
                            .with_header("Content-Type", "text/plain")
                            .with_header("Content-Length", &CHECKSUMS_LENGTH.to_string())
                            .ok();
                    } else {
                        let mut string = String::new();
                        string.reserve_exact(CHECKSUMS_LENGTH as usize);

                        string.push_str(&format!(
                            "md5    {}\n",
                            file.md5
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>()
                        ));
                        string.push_str(&format!(
                            "sha1   {}\n",
                            file.sha1
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>()
                        ));
                        string.push_str(&format!(
                            "sha224 {}\n",
                            file.sha224
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>()
                        ));
                        string.push_str(&format!(
                            "sha256 {}\n",
                            file.sha256
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>()
                        ));
                        string.push_str(&format!(
                            "sha384 {}\n",
                            file.sha384
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>()
                        ));
                        string.push_str(&format!(
                            "sha512 {}\n",
                            file.sha512
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>()
                        ));

                        return ApiResponse::new(Body::from(string))
                            .with_header("Content-Type", "text/plain")
                            .ok();
                    }
                }

                let file = match File::by_path(&state.database, &state.cache, path).await? {
                    Some(file) => file,
                    None => {
                        log_complete(
                            &state,
                            &parts,
                            organization,
                            CompletedFile {
                                kind: FileRequestKind::File,
                                path,
                                size: 0,
                                bytes_sent: 0,
                                status: StatusCode::NOT_FOUND,
                                started,
                            },
                        )
                        .await;

                        return render_not_found(&state, &format!("/{}", path.to_string_lossy()));
                    }
                };

                if parts.method == Method::HEAD {
                    log_complete(
                        &state,
                        &parts,
                        organization,
                        CompletedFile {
                            kind: FileRequestKind::File,
                            path,
                            size: file.size,
                            bytes_sent: 0,
                            status: StatusCode::OK,
                            started,
                        },
                    )
                    .await;

                    return ApiResponse::new(Body::empty())
                        .with_header(
                            "Content-Type",
                            if last.ends_with(".jar") {
                                "application/java-archive"
                            } else {
                                "application/zip"
                            },
                        )
                        .with_header("Content-Length", &file.size.to_string())
                        .with_header(
                            "ETag",
                            &file
                                .sha256
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>(),
                        )
                        .with_header("Cache-Control", "public, max-age=604800")
                        .ok();
                } else {
                    let opened = state.files.get(path, &file).await;

                    let id = state
                        .requests
                        .log_file(
                            &parts,
                            organization,
                            FileRequestKind::File,
                            path,
                            file.size,
                            opened.as_ref().is_ok_and(|(hit, _)| *hit),
                        )
                        .await;

                    let (_, file_reader) = match opened {
                        Ok(opened) => opened,
                        Err(err) => {
                            state
                                .requests
                                .finish_file(
                                    id,
                                    StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i16,
                                    started.elapsed().as_millis() as i32,
                                    0,
                                )
                                .await;

                            return Err(err.into());
                        }
                    };

                    let body = Body::from_stream(TrackedFileStream::new(
                        tokio_util::io::ReaderStream::new(file_reader),
                        state.0.clone(),
                        id,
                        StatusCode::OK.as_u16() as i16,
                        started,
                    ));

                    return ApiResponse::new(body)
                        .with_header(
                            "Content-Type",
                            if last.ends_with(".jar") {
                                "application/java-archive"
                            } else {
                                "application/zip"
                            },
                        )
                        .with_header("Content-Length", &file.size.to_string())
                        .with_header(
                            "ETag",
                            &file
                                .sha256
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>(),
                        )
                        .with_header("Cache-Control", "public, max-age=604800")
                        .ok();
                }
            }

            let files = File::all_for_root(&state.database, &state.cache, path).await?;

            let mut index_files = Vec::new();
            index_files.reserve_exact(
                files
                    .iter()
                    .map(|f| if f.is_directory { 1 } else { 2 })
                    .sum(),
            );

            for f in files {
                index_files.push(IndexFile {
                    name: compact_str::format_compact!(
                        "{}{}",
                        f.name,
                        if f.is_directory { "/" } else { "" }
                    ),
                    size: human_bytes::human_bytes(f.size as f64).into(),
                    href: Some(compact_str::format_compact!(
                        "{}{}",
                        f.name,
                        if f.is_directory { "/" } else { "" }
                    )),
                });

                if !f.is_directory {
                    index_files.push(IndexFile {
                        name: compact_str::format_compact!("{}.CHECKSUMS.txt", f.name),
                        size: human_bytes::human_bytes(CHECKSUMS_LENGTH as f64).into(),
                        href: Some(compact_str::format_compact!("{}.CHECKSUMS.txt", f.name)),
                    });
                }
            }

            let missing = index_files.is_empty() && !path.as_os_str().is_empty();

            log_complete(
                &state,
                &parts,
                organization,
                CompletedFile {
                    kind: FileRequestKind::Index,
                    path,
                    size: 0,
                    bytes_sent: 0,
                    status: if missing {
                        StatusCode::NOT_FOUND
                    } else {
                        StatusCode::OK
                    },
                    started,
                },
            )
            .await;

            let location = compact_str::format_compact!("/{}", path.to_string_lossy());

            if missing {
                return render_not_found(&state, &location);
            }

            render(&state, &location, index_files)
        }))
        .with_state(state.clone())
}
