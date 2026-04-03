use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;

pub const DEFAULT_PAGE_SIZE: i64 = 20;

pub trait Paginate {
    fn cursor_value(&self) -> String;
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

pub fn paginate<T: Serialize + Paginate>(mut rows: Vec<T>, limit: i64) -> PaginatedResponse<T> {
    let has_more = rows.len() as i64 > limit;
    rows.truncate(limit as usize);
    let next_cursor = if has_more {
        rows.last().map(|r| r.cursor_value())
    } else {
        None
    };
    PaginatedResponse {
        items: rows,
        next_cursor,
        has_more,
    }
}

/// Parse a "timestamp\0key" cursor string into its components.
pub fn parse_cursor(cursor: Option<&str>) -> (Option<String>, Option<String>) {
    match cursor {
        Some(c) => match c.split_once('\0') {
            Some((ts, key)) => (Some(ts.to_string()), Some(key.to_string())),
            None => (None, None),
        },
        None => (None, None),
    }
}

// -- Memory types --

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    User,
    Feedback,
    Project,
    Reference,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "feedback" => Some(Self::Feedback),
            "project" => Some(Self::Project),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MemoryRow {
    pub key: String,
    pub content: String,
    pub memory_type: String,
    pub project: Option<String>,
    pub tags: String, // JSON array
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl Paginate for MemoryRow {
    fn cursor_value(&self) -> String {
        format_ts_key_cursor(&self.updated_at, &self.key)
    }
}

// -- Artifact types --

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ArtifactRow {
    pub key: String,
    pub content: String,
    pub artifact_type: String,
    pub project: Option<String>,
    pub source_agent: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
}

impl Paginate for ArtifactRow {
    fn cursor_value(&self) -> String {
        format_ts_key_cursor(&self.created_at, &self.key)
    }
}

// -- Event types --

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct EventRow {
    pub id: i64,
    pub source_agent: String,
    pub event_type: String,
    pub payload: String, // JSON object
    pub project: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl Paginate for EventRow {
    fn cursor_value(&self) -> String {
        self.id.to_string()
    }
}

fn format_ts_key_cursor(ts: &OffsetDateTime, key: &str) -> String {
    format!(
        "{}\0{}",
        ts.format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        key
    )
}
