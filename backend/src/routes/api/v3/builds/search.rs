use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod post {
    use crate::{
        models::{
            BaseModel,
            build::Build,
            r#type::ServerType,
            version::{MinifiedVersion, VersionType},
        },
        response::{ApiResponse, ApiResponseResult},
        routes::{ApiErrorV3, GetData, GetState},
    };
    use axum::http::StatusCode;
    use axum_extra::extract::Query;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use sqlx::Row;
    use utoipa::ToSchema;

    #[derive(ToSchema, Deserialize)]
    pub struct Params {
        #[serde(default)]
        fields: Vec<compact_str::CompactString>,
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    pub struct Hash {
        primary: Option<bool>,
        sha1: Option<String>,
        sha224: Option<String>,
        sha256: Option<String>,
        sha384: Option<String>,
        sha512: Option<String>,
        md5: Option<String>,
    }

    impl Hash {
        #[inline]
        pub fn has_any(&self) -> bool {
            self.sha1.is_some()
                || self.sha224.is_some()
                || self.sha256.is_some()
                || self.sha384.is_some()
                || self.sha512.is_some()
                || self.md5.is_some()
        }

        #[inline]
        pub fn normalize(&mut self) -> bool {
            for (hash, length) in [
                (&mut self.sha1, 40),
                (&mut self.sha224, 56),
                (&mut self.sha256, 64),
                (&mut self.sha384, 96),
                (&mut self.sha512, 128),
                (&mut self.md5, 32),
            ] {
                let Some(hash) = hash else {
                    continue;
                };

                if !crate::utils::is_hex(hash, length) {
                    return false;
                }

                hash.make_ascii_lowercase();
            }

            true
        }
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    pub struct BuildSearch {
        uuid: Option<uuid::Uuid>,
        r#type: Option<ServerType>,
        version_id: Option<String>,
        project_version_id: Option<String>,
        name: Option<String>,
        experimental: Option<bool>,

        #[schema(inline)]
        hash: Option<Hash>,
    }

    impl BuildSearch {
        #[inline]
        pub fn has_any(&self) -> bool {
            self.uuid.is_some()
                || self.r#type.is_some()
                || self.version_id.is_some()
                || self.project_version_id.is_some()
                || self.name.is_some()
                || self.experimental.is_some()
                || self.hash.as_ref().map(|h| h.has_any()).unwrap_or(false)
        }

        #[inline]
        pub fn normalize(&mut self) -> bool {
            self.hash.as_mut().map(Hash::normalize).unwrap_or(true)
        }
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    struct RawBuildResult {
        build: Build,
        latest: Build,
        version: MinifiedVersion,
    }

    impl RawBuildResult {
        fn into_response_build(self) -> ResponseBuild {
            ResponseBuild {
                build: self.build.into_api_v3(),
                latest: self.latest.into_api_v3(),
                version: self.version.into_api_v3(),
            }
        }
    }

    #[derive(ToSchema, Serialize, Deserialize)]
    struct ResponseBuild {
        build: crate::models::build::ApiBuildV3,
        latest: crate::models::build::ApiBuildV3,
        version: crate::models::version::ApiMinifiedVersionV3,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        #[schema(inline)]
        builds: Vec<Option<ResponseBuild>>,
    }

    #[utoipa::path(post, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = inline(ApiErrorV3)),
        (status = NOT_FOUND, body = inline(ApiErrorV3)),
    ), params(
        (
            "fields" = Vec<String>, Query,
            description = "HTML form data array of build fields to include in the response (e.g. fields=created&fields=java)",
        )
    ), request_body = inline(Vec<BuildSearch>))]
    pub async fn route(
        state: GetState,
        request_data: GetData,
        params: Query<Params>,
        crate::Payload(data): crate::Payload<Vec<BuildSearch>>,
    ) -> ApiResponseResult {
        if data.len() > 10 {
            return ApiResponse::error("you can only search for up to 10 builds at a time")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        let mut results = Vec::with_capacity(data.len());
        for search in data {
            results.push(lookup_build(&state.database, &state.cache, search));
        }

        let results = futures_util::future::try_join_all(results).await?;

        if let Some(result) = results.iter().flatten().next() {
            *request_data.lock().unwrap() = json!({
                "type": "lookup",
                "build": {
                    "id": result.build.id,
                    "type": result.build.r#type,
                    "versionId": result.build.version_id,
                    "projectVersionId": result.build.project_version_id,
                    "buildNumber": result.build.build_number,
                    "java": result.version.java,
                }
            });
        }

        ApiResponse::new_serialized(json!({
            "success": true,
            "builds": results
                .into_iter()
                .map(|r| r.map(|result| result.into_response_build()))
                .map(|r| {
                    r.map(|result| {
                        json!({
                            "build": crate::utils::extract_fields(result.build, &params.fields),
                            "latest": crate::utils::extract_fields(result.latest, &params.fields),
                            "version": result.version,
                        })
                    })
                })
                .collect::<Vec<_>>()
        }))
        .ok()
    }

    #[inline]
    async fn lookup_build(
        database: &crate::database::Database,
        cache: &crate::cache::Cache,
        mut search: BuildSearch,
    ) -> Result<Option<RawBuildResult>, anyhow::Error> {
        if !search.has_any() || !search.normalize() {
            return Ok(None);
        }

        cache.cached(&format!("build::search::{}::v3", crate::utils::cache_key_hash(&serde_json::to_string(&search)?)), 3600, || async {
            let mut where_clause: Vec<String> = Vec::new();
            let mut data: Vec<serde_json::Value> = Vec::new();

            if let Some(uuid) = search.uuid {
                where_clause.push(format!("builds.uuid = ($1->>{})::uuid", data.len()));
                data.push(serde_json::to_value(uuid)?);
            }
            if let Some(r#type) = search.r#type {
                where_clause.push(format!("builds.type = ($1->>{})::server_type", data.len()));
                data.push(serde_json::to_value(r#type)?);
            }
            if let Some(version_id) = &search.version_id {
                where_clause.push(format!("builds.version_id = $1->>{}", data.len()));
                data.push(serde_json::to_value(version_id)?);
            }
            if let Some(project_version_id) = &search.project_version_id {
                where_clause.push(format!("builds.project_version_id = $1->>{}", data.len()));
                data.push(serde_json::to_value(project_version_id)?);
            }
            if let Some(name) = &search.name {
                where_clause.push(format!("builds.name = $1->>{}", data.len()));
                data.push(serde_json::to_value(name)?);
            }
            if let Some(experimental) = search.experimental {
                where_clause.push(format!("builds.experimental = ($1->>{})::bool", data.len()));
                data.push(serde_json::to_value(experimental)?);
            }
            if let Some(hash) = &search.hash && hash.has_any() {
                if let Some(primary) = hash.primary {
                    where_clause.push(format!("build_hashes.primary = ($1->>{})::bool", data.len()));
                    data.push(serde_json::to_value(primary)?);
                }
                if let Some(sha1) = &hash.sha1 {
                    where_clause.push(format!("build_hashes.sha1 = decode($1->>{}, 'hex')", data.len()));
                    data.push(serde_json::to_value(sha1)?);
                }
                if let Some(sha224) = &hash.sha224 {
                    where_clause.push(format!("build_hashes.sha224 = decode($1->>{}, 'hex')", data.len()));
                    data.push(serde_json::to_value(sha224)?);
                }
                if let Some(sha256) = &hash.sha256 {
                    where_clause.push(format!("build_hashes.sha256 = decode($1->>{}, 'hex')", data.len()));
                    data.push(serde_json::to_value(sha256)?);
                }
                if let Some(sha384) = &hash.sha384 {
                    where_clause.push(format!("build_hashes.sha384 = decode($1->>{}, 'hex')", data.len()));
                    data.push(serde_json::to_value(sha384)?);
                }
                if let Some(sha512) = &hash.sha512 {
                    where_clause.push(format!("build_hashes.sha512 = decode($1->>{}, 'hex')", data.len()));
                    data.push(serde_json::to_value(sha512)?);
                }
                if let Some(md5) = &hash.md5 {
                    where_clause.push(format!("build_hashes.md5 = decode($1->>{}, 'hex')", data.len()));
                    data.push(serde_json::to_value(md5)?);
                }
            }

            let query = sqlx::query(sqlx::AssertSqlSafe(format!(
                r#"
                WITH spec_build AS (
                    SELECT {}
                    FROM {}
                    WHERE {}
                    ORDER BY builds.id DESC
                    LIMIT 1
                ),
    
                filtered_builds AS (
                    SELECT {}
                    FROM builds b
                    INNER JOIN spec_build sb ON
                        sb.id = b.id
                        OR (b.version_id = COALESCE(sb.version_id, sb.project_version_id) AND sb.type = b.type)
                        OR (b.version_id IS NULL AND b.project_version_id = COALESCE(sb.version_id, sb.project_version_id) AND sb.type = b.type)
                    WHERE b.type <> 'ARCLIGHT'
                        OR split_part(sb.project_version_id, '-', -1) = split_part(b.project_version_id, '-', -1)
                        OR split_part(sb.project_version_id, '-', -1) NOT IN ('forge', 'neoforge', 'fabric')
                ),

                build_count AS (
                    SELECT
                        (
                            SELECT count(*) FROM builds
                            WHERE version_id = (SELECT COALESCE(version_id, project_version_id) FROM spec_build)
                        ) + (
                            SELECT count(*) FROM builds
                            WHERE version_id IS NULL
                                AND project_version_id = (SELECT COALESCE(version_id, project_version_id) FROM spec_build)
                        ) AS count
                )
    
                SELECT
                    *,
                    0 AS build_count,
                    now()::timestamp as version2_created,
                    'RELEASE'::version_type AS version_type,
                    false AS version_supported,
                    0 AS version_java,
                    now()::timestamp AS version_created
                FROM spec_build
    
                UNION ALL
    
                SELECT
                    x.*,
                    mv.type AS version_type,
                    mv.supported AS version_supported,
                    mv.java AS version_java,
                    mv.created AS version_created
                FROM (
                    SELECT *
                    FROM (
                        SELECT
                            {},
                            (SELECT count FROM build_count) AS build_count,
                            min(b.created) OVER () AS version2_created
                        FROM filtered_builds b
                        ORDER BY b.id DESC
                    ) LIMIT 1
                ) x
                LEFT JOIN minecraft_versions mv ON mv.id = x.version_id
                "#,
                Build::columns_sql(None, None),
                if search.hash.as_ref().map(|h| h.has_any()).unwrap_or(false) {
                    "build_hashes INNER JOIN builds ON builds.id = build_hashes.build_id"
                } else {
                    "builds"
                },
                where_clause.join(" AND "),
                Build::columns_sql(None, Some("b")),
                Build::columns_sql(None, Some("b"))
            )))
            .bind(serde_json::to_value(data).unwrap())
            .fetch_all(database.read())
            .await?;

            if query.len() != 2 {
                return Ok(None);
            }

            Ok::<_, anyhow::Error>(Some(RawBuildResult {
                build: Build::map(None, &query[0])?,
                latest: Build::map(None, &query[1])?,
                version: MinifiedVersion {
                    id: query[1]
                        .try_get("version_id")
                        .unwrap_or_else(|_| query[1].get("project_version_id")),
                    r#type: query[1].try_get("version_type").unwrap_or(VersionType::Release),
                    supported: query[1].try_get("version_supported").unwrap_or(true),
                    java: query[1].try_get("version_java").unwrap_or(21),
                    builds: query[1].try_get("build_count").unwrap_or(0),
                    created: query[1]
                        .try_get("version_created")
                        .unwrap_or(query[1].try_get("version2_created").unwrap_or_default()),
                },
            }))
        })
        .await
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(post::route))
        .with_state(state.clone())
}
