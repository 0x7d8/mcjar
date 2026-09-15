use super::{BaseModel, r#type::ServerType, version::VersionType};
use crate::prelude::IteratorExtension;
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow, types::chrono::NaiveDateTime};
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(ToSchema, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum InstallationStep {
    #[serde(rename = "download")]
    Download(#[schema(inline)] InstallationStepDownload),
    #[serde(rename = "unzip")]
    Unzip(#[schema(inline)] InstallationStepUnzip),
    #[serde(rename = "remove")]
    Remove(#[schema(inline)] InstallationStepRemove),
}

#[derive(ToSchema, Serialize, Deserialize, Clone)]
pub struct InstallationStepDownload {
    pub url: compact_str::CompactString,
    pub file: compact_str::CompactString,
    pub size: u64,
}
#[derive(ToSchema, Serialize, Deserialize, Clone)]
pub struct InstallationStepUnzip {
    pub file: compact_str::CompactString,
    pub location: compact_str::CompactString,
}
#[derive(ToSchema, Serialize, Deserialize, Clone)]
pub struct InstallationStepRemove {
    pub location: compact_str::CompactString,
}

#[derive(ToSchema, Serialize, Deserialize, Clone)]
pub struct Build {
    pub id: i32,
    pub uuid: uuid::Uuid,

    #[serde(rename(serialize = "versionId"), alias = "versionId")]
    #[schema(rename = "versionId", example = "1.17.1")]
    pub version_id: Option<compact_str::CompactString>,
    #[serde(rename(serialize = "projectVersionId"), alias = "projectVersionId")]
    #[schema(rename = "projectVersionId")]
    pub project_version_id: Option<compact_str::CompactString>,

    pub r#type: ServerType,
    pub experimental: bool,

    pub name: compact_str::CompactString,
    #[serde(rename(serialize = "buildNumber"), alias = "buildNumber")]
    #[schema(rename = "buildNumber")]
    pub build_number: i32,
    #[serde(rename(serialize = "jarUrl"), alias = "jarUrl")]
    #[schema(rename = "jarUrl")]
    pub jar_url: Option<compact_str::CompactString>,
    #[serde(rename(serialize = "jarSize"), alias = "jarSize")]
    #[schema(rename = "jarSize")]
    pub jar_size: Option<i32>,
    #[serde(rename(serialize = "zipUrl"), alias = "zipUrl")]
    #[schema(rename = "zipUrl")]
    pub zip_url: Option<compact_str::CompactString>,
    #[serde(rename(serialize = "zipSize"), alias = "zipSize")]
    #[schema(rename = "zipSize")]
    pub zip_size: Option<i32>,

    pub installation: Vec<Vec<InstallationStep>>,
    pub changes: Vec<compact_str::CompactString>,

    pub created: Option<NaiveDateTime>,
}

impl BaseModel for Build {
    fn columns(
        prefix: Option<&str>,
        table: Option<&str>,
    ) -> BTreeMap<compact_str::CompactString, compact_str::CompactString> {
        let table = table.unwrap_or("builds");

        BTreeMap::from([
            (
                compact_str::format_compact!("{table}.id"),
                compact_str::format_compact!("{}id", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.uuid"),
                compact_str::format_compact!("{}uuid", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.version_id"),
                compact_str::format_compact!("{}version_id", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.project_version_id"),
                compact_str::format_compact!("{}project_version_id", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.type"),
                compact_str::format_compact!("{}type", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.experimental"),
                compact_str::format_compact!("{}experimental", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.name"),
                compact_str::format_compact!("{}name", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.build_number"),
                compact_str::format_compact!("{}build_number", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.jar_url"),
                compact_str::format_compact!("{}jar_url", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.jar_size"),
                compact_str::format_compact!("{}jar_size", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.zip_url"),
                compact_str::format_compact!("{}zip_url", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.zip_size"),
                compact_str::format_compact!("{}zip_size", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.installation"),
                compact_str::format_compact!("{}installation", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.changes"),
                compact_str::format_compact!("{}changes", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.created"),
                compact_str::format_compact!("{}created", prefix.unwrap_or_default()),
            ),
        ])
    }

    fn map(prefix: Option<&str>, row: &PgRow) -> Result<Self, anyhow::Error> {
        let prefix = prefix.unwrap_or_default();

        Ok(Self {
            id: row.try_get(compact_str::format_compact!("{prefix}id").as_str())?,
            uuid: row.try_get(compact_str::format_compact!("{prefix}uuid").as_str())?,
            version_id: row
                .try_get(compact_str::format_compact!("{prefix}version_id").as_str())
                .ok(),
            project_version_id: row
                .try_get(compact_str::format_compact!("{prefix}project_version_id").as_str())
                .ok(),
            r#type: row.try_get(compact_str::format_compact!("{prefix}type").as_str())?,
            experimental: row
                .try_get(compact_str::format_compact!("{prefix}experimental").as_str())?,
            name: row.try_get(compact_str::format_compact!("{prefix}name").as_str())?,
            build_number: row
                .try_get(compact_str::format_compact!("{prefix}build_number").as_str())?,
            jar_url: row.try_get(compact_str::format_compact!("{prefix}jar_url").as_str())?,
            jar_size: row.try_get(compact_str::format_compact!("{prefix}jar_size").as_str())?,
            zip_url: row.try_get(compact_str::format_compact!("{prefix}zip_url").as_str())?,
            zip_size: row.try_get(compact_str::format_compact!("{prefix}zip_size").as_str())?,
            installation: serde_json::from_value(
                row.try_get(compact_str::format_compact!("{prefix}installation").as_str())?,
            )?,
            changes: serde_json::from_value(
                row.try_get(compact_str::format_compact!("{prefix}changes").as_str())?,
            )?,
            created: row.try_get(compact_str::format_compact!("{prefix}created").as_str())?,
        })
    }
}

impl Build {
    #[inline]
    pub fn installation_size(&self) -> u64 {
        self.installation
            .iter()
            .flat_map(|step| step.iter())
            .filter_map(|step| match step {
                InstallationStep::Download(step) => Some(step.size),
                _ => None,
            })
            .sum()
    }

    pub async fn by_identifier(
        database: &crate::database::Database,
        cache: &crate::cache::Cache,
        identifier: &str,
    ) -> Result<Option<(Self, Self, super::version::MinifiedVersion)>, anyhow::Error> {
        let hash: Option<&str> = match identifier.len() {
            32 => Some("md5"),
            40 => Some("sha1"),
            56 => Some("sha224"),
            64 => Some("sha256"),
            96 => Some("sha384"),
            128 => Some("sha512"),
            _ => {
                if let Ok(id) = identifier.parse::<i32>() {
                    if id < 1 {
                        return Ok(None);
                    } else {
                        None
                    }
                } else if identifier.parse::<uuid::Uuid>().is_ok() {
                    None
                } else {
                    return Ok(None);
                }
            }
        };

        if hash.is_some() && !identifier.chars().all(|char| char.is_ascii_hexdigit()) {
            return Ok(None);
        }

        let identifier = &identifier.to_ascii_lowercase();

        cache.cached(&format!("build::identifier::{identifier}"), 3600, || async {
            let query = sqlx::query(sqlx::AssertSqlSafe(format!(
                r#"
                WITH spec_build AS (
                    SELECT {}
                    FROM {}
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
                    'RELEASE' AS version_type,
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
                Self::columns_sql(None, None),
                if let Some(hash) = hash {
                    compact_str::format_compact!("build_hashes INNER JOIN builds ON builds.id = build_hashes.build_id WHERE {hash} = decode($1, 'hex')")
                } else if identifier.parse::<uuid::Uuid>().is_ok() {
                    compact_str::format_compact!("builds WHERE uuid = $1::uuid")
                } else {
                    "builds WHERE builds.id = $1::int".into()
                },
                Self::columns_sql(None, Some("b")),
                Self::columns_sql(None, Some("b"))
            )))
            .bind(identifier)
            .fetch_all(database.read())
            .await?;

            if query.len() != 2 {
                return Ok(None);
            }

            Ok::<_, anyhow::Error>(Some((
                Self::map(None, &query[0])?,
                Self::map(None, &query[1])?,
                super::version::MinifiedVersion {
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
            )))})
        .await
    }

    pub async fn by_build_number(
        database: &crate::database::Database,
        r#type: ServerType,
        version_location: &str,
        version_id: &str,
        build_number: Option<i32>,
    ) -> Result<Option<Self>, anyhow::Error> {
        let data = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM builds
            WHERE
                {} = $1
                AND type = $2
                {}
            ORDER BY id DESC
            "#,
            Self::columns_sql(None, None),
            version_location,
            if build_number.is_some() {
                "AND build_number = $3"
            } else {
                ""
            }
        )))
        .bind(version_id)
        .bind(r#type)
        .bind(build_number)
        .fetch_optional(database.read())
        .await?;

        data.map(|row| Self::map(None, &row)).transpose()
    }

    pub async fn all_by_version(
        database: &crate::database::Database,
        r#type: ServerType,
        version_location: &str,
        version_id: &str,
    ) -> Result<Vec<Self>, anyhow::Error> {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM builds
            WHERE
                {} = $1
                AND type = $2
            ORDER BY id DESC
            "#,
            Self::columns_sql(None, None),
            version_location
        )))
        .bind(version_id)
        .bind(r#type)
        .fetch_all(database.read())
        .await?
        .into_iter()
        .map(|row| Self::map(None, &row))
        .try_collect_vec()
    }

    pub async fn all_by_version_with_pagination(
        database: &crate::database::Database,
        r#type: ServerType,
        version_location: &str,
        version_id: &str,
        page: i64,
        per_page: i64,
        search: Option<&str>,
    ) -> Result<crate::models::Pagination<Self>, anyhow::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}, COUNT(*) OVER() AS total_count
            FROM builds
            WHERE
                {} = $1
                AND type = $2
                AND ($5 IS NULL OR name ILIKE '%' || $5 || '%')
            ORDER BY id DESC
            LIMIT $3 OFFSET $4
            "#,
            Self::columns_sql(None, None),
            version_location
        )))
        .bind(version_id)
        .bind(r#type)
        .bind(per_page)
        .bind((page - 1) * per_page)
        .bind(search)
        .fetch_all(database.read())
        .await?;

        Ok(super::Pagination {
            total: rows
                .first()
                .map_or(Ok(0), |row| row.try_get("total_count"))?,
            per_page,
            page,
            data: rows
                .into_iter()
                .map(|row| Self::map(None, &row))
                .try_collect_vec()?,
        })
    }

    pub async fn all_by_minecraft_version(
        database: &crate::database::Database,
        version_id: &str,
    ) -> Result<Vec<Self>, anyhow::Error> {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {}
            FROM builds
            WHERE version_id = $1
            ORDER BY id DESC
            "#,
            Self::columns_sql(None, None)
        )))
        .bind(version_id)
        .fetch_all(database.read())
        .await?
        .into_iter()
        .map(|row| Self::map(None, &row))
        .try_collect_vec()
    }

    pub fn into_api_v3(self) -> ApiBuildV3 {
        ApiBuildV3 {
            uuid: self.uuid,
            version_id: self.version_id,
            project_version_id: self.project_version_id,
            r#type: self.r#type,
            experimental: self.experimental,
            name: self.name,
            installation: self.installation,
            changes: self.changes,
            created: self.created.map(|dt| dt.and_utc()),
        }
    }
}

#[derive(ToSchema, Serialize, Deserialize)]
#[schema(title = "BuildV3")]
pub struct ApiBuildV3 {
    pub uuid: uuid::Uuid,
    pub version_id: Option<compact_str::CompactString>,
    pub project_version_id: Option<compact_str::CompactString>,

    pub r#type: ServerType,
    pub experimental: bool,

    pub name: compact_str::CompactString,

    pub installation: Vec<Vec<InstallationStep>>,
    pub changes: Vec<compact_str::CompactString>,

    pub created: Option<chrono::DateTime<chrono::Utc>>,
}
