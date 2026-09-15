use super::BaseModel;
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow, types::chrono::NaiveDateTime};
use std::collections::BTreeMap;
use utoipa::ToSchema;

pub const ALIVE_WINDOW_SECONDS: i64 = 60;

#[derive(ToSchema, Serialize, Deserialize, Clone)]
pub struct Node {
    pub name: compact_str::CompactString,

    pub url: compact_str::CompactString,
    pub version: compact_str::CompactString,

    pub last_seen: NaiveDateTime,
    pub created: NaiveDateTime,
}

impl BaseModel for Node {
    fn columns(
        prefix: Option<&str>,
        table: Option<&str>,
    ) -> BTreeMap<compact_str::CompactString, compact_str::CompactString> {
        let table = table.unwrap_or("nodes");

        BTreeMap::from([
            (
                compact_str::format_compact!("{table}.name"),
                compact_str::format_compact!("{}name", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.url"),
                compact_str::format_compact!("{}url", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.version"),
                compact_str::format_compact!("{}version", prefix.unwrap_or_default()),
            ),
            (
                compact_str::format_compact!("{table}.last_seen"),
                compact_str::format_compact!("{}last_seen", prefix.unwrap_or_default()),
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
            name: row.try_get(compact_str::format_compact!("{prefix}name").as_str())?,
            url: row.try_get(compact_str::format_compact!("{prefix}url").as_str())?,
            version: row.try_get(compact_str::format_compact!("{prefix}version").as_str())?,
            last_seen: row.try_get(compact_str::format_compact!("{prefix}last_seen").as_str())?,
            created: row.try_get(compact_str::format_compact!("{prefix}created").as_str())?,
        })
    }
}

impl Node {
    pub async fn heartbeat(
        database: &crate::database::Database,
        name: &str,
        url: &str,
        version: &str,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            INSERT INTO nodes (name, url, version, last_seen)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (name) DO UPDATE
            SET url = $2, version = $3, last_seen = NOW()
            "#,
        )
        .bind(name)
        .bind(url)
        .bind(version)
        .execute(database.write())
        .await?;

        Ok(())
    }

    pub async fn all(database: &crate::database::Database) -> Result<Vec<Self>, anyhow::Error> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            SELECT {} FROM nodes
            ORDER BY nodes.name ASC
            "#,
            Self::columns_sql(None, None)
        )))
        .fetch_all(database.read())
        .await?;

        rows.iter().map(|row| Self::map(None, row)).collect()
    }

    #[inline]
    pub fn alive(&self) -> bool {
        self.last_seen
            > chrono::Utc::now().naive_utc() - chrono::Duration::seconds(ALIVE_WINDOW_SECONDS)
    }
}
