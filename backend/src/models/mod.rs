use compact_str::CompactStringExt;
use garde::Validate;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::postgres::PgRow;
use std::collections::BTreeMap;
use utoipa::ToSchema;

pub mod build;
pub mod config;
pub mod file;
pub mod node;
pub mod organization;
pub mod r#type;
pub mod user;
pub mod version;

#[derive(ToSchema, Validate, Deserialize)]
pub struct PaginationParams {
    #[garde(range(min = 1))]
    #[schema(minimum = 1)]
    #[serde(default = "Pagination::default_page")]
    pub page: i64,
    #[garde(range(min = 1, max = 200))]
    #[schema(minimum = 1, maximum = 200)]
    #[serde(default = "Pagination::default_per_page")]
    pub per_page: i64,
}

#[derive(ToSchema, Validate, Deserialize)]
pub struct PaginationParamsWithSearch {
    #[garde(range(min = 1))]
    #[schema(minimum = 1)]
    #[serde(default = "Pagination::default_page")]
    pub page: i64,
    #[garde(range(min = 1, max = 200))]
    #[schema(minimum = 1, maximum = 200)]
    #[serde(default = "Pagination::default_per_page")]
    pub per_page: i64,
    #[garde(length(chars, min = 1, max = 128))]
    #[schema(min_length = 1, max_length = 128)]
    #[serde(
        default,
        deserialize_with = "crate::deserialize::deserialize_string_option"
    )]
    pub search: Option<compact_str::CompactString>,
}

#[derive(ToSchema, Validate, Deserialize)]
pub struct PaginationParamsWithSearchAndFields {
    #[garde(range(min = 1))]
    #[schema(minimum = 1)]
    #[serde(default = "Pagination::default_page")]
    pub page: i64,
    #[garde(range(min = 1, max = 200))]
    #[schema(minimum = 1, maximum = 200)]
    #[serde(default = "Pagination::default_per_page")]
    pub per_page: i64,
    #[garde(length(chars, min = 1, max = 128))]
    #[schema(min_length = 1, max_length = 128)]
    #[serde(
        default,
        deserialize_with = "crate::deserialize::deserialize_string_option"
    )]
    pub search: Option<compact_str::CompactString>,
    #[garde(skip)]
    #[serde(default)]
    pub fields: Vec<compact_str::CompactString>,
}

#[derive(ToSchema, Serialize, Deserialize)]
pub struct Pagination<T: Serialize = serde_json::Value> {
    pub total: i64,
    pub per_page: i64,
    pub page: i64,

    pub data: Vec<T>,
}

impl Pagination {
    #[inline]
    pub const fn default_page() -> i64 {
        1
    }

    #[inline]
    pub const fn default_per_page() -> i64 {
        50
    }
}

impl<T: Serialize> Pagination<T> {
    pub fn map<R: Serialize>(self, f: impl FnMut(T) -> R) -> Pagination<R> {
        Pagination {
            total: self.total,
            per_page: self.per_page,
            page: self.page,
            data: self.data.into_iter().map(f).collect(),
        }
    }
}

pub trait BaseModel: Serialize + DeserializeOwned {
    fn columns(
        prefix: Option<&str>,
        table: Option<&str>,
    ) -> BTreeMap<compact_str::CompactString, compact_str::CompactString>;

    #[inline]
    fn columns_sql(prefix: Option<&str>, table: Option<&str>) -> compact_str::CompactString {
        Self::columns(prefix, table)
            .iter()
            .map(|(key, value)| compact_str::format_compact!("{key} as {value}"))
            .collect::<Vec<compact_str::CompactString>>()
            .join_compact(", ")
    }

    fn map(prefix: Option<&str>, row: &PgRow) -> Result<Self, anyhow::Error>;
}
