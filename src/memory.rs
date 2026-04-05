use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::schema::{
    ContentDetail, DEFAULT_PAGE_SIZE, MemoryRow, MemorySummaryRow, MemoryType, MemoryVersionRow,
    PaginatedResponse, paginate, parse_cursor,
};

const PREVIEW_LENGTH: usize = 100;

pub struct ListParams<'a> {
    pub memory_type: Option<MemoryType>,
    pub project: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub cursor: Option<&'a str>,
    pub limit: Option<i64>,
    pub detail: ContentDetail,
}

fn preview(s: &str) -> String {
    if s.len() <= PREVIEW_LENGTH {
        s.to_string()
    } else {
        format!("{}...", &s[..s.floor_char_boundary(PREVIEW_LENGTH)])
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status")]
pub enum WriteResult {
    #[serde(rename = "ok")]
    Ok { row: MemoryRow },
    #[serde(rename = "conflict")]
    Conflict {
        key: String,
        expected_version: i64,
        current_version: i64,
        current_content_preview: String,
    },
}

pub async fn write(
    pool: &SqlitePool,
    key: &str,
    content: &str,
    memory_type: MemoryType,
    project: Option<&str>,
    tags: &str,
    version: i64,
) -> Result<WriteResult> {
    let existing = sqlx::query_as::<_, MemoryRow>("SELECT * FROM memories WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .context("failed to check existing memory")?;

    match existing {
        None => {
            // New key — version must be 1
            if version != 1 {
                return Ok(WriteResult::Conflict {
                    key: key.to_string(),
                    expected_version: version,
                    current_version: 0,
                    current_content_preview: String::new(),
                });
            }
            let row = sqlx::query_as::<_, MemoryRow>(
                "INSERT INTO memories (key, content, memory_type, project, tags, version)
                 VALUES ($1, $2, $3, $4, $5, 1)
                 RETURNING *",
            )
            .bind(key)
            .bind(content)
            .bind(memory_type.as_str())
            .bind(project)
            .bind(tags)
            .fetch_one(pool)
            .await
            .context("failed to insert memory")?;
            Ok(WriteResult::Ok { row })
        }
        Some(current) => {
            // Existing key — version must be current + 1
            if version != current.version + 1 {
                return Ok(WriteResult::Conflict {
                    key: key.to_string(),
                    expected_version: version,
                    current_version: current.version,
                    current_content_preview: preview(&current.content),
                });
            }
            // UPDATE triggers the version history snapshot
            let row = sqlx::query_as::<_, MemoryRow>(
                "UPDATE memories SET
                     content = $2,
                     memory_type = $3,
                     project = $4,
                     tags = $5,
                     version = $6,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE key = $1
                 RETURNING *",
            )
            .bind(key)
            .bind(content)
            .bind(memory_type.as_str())
            .bind(project)
            .bind(tags)
            .bind(version)
            .fetch_one(pool)
            .await
            .context("failed to update memory")?;
            Ok(WriteResult::Ok { row })
        }
    }
}

pub async fn read(pool: &SqlitePool, key: &str) -> Result<Option<MemoryRow>> {
    sqlx::query_as::<_, MemoryRow>("SELECT * FROM memories WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .context("failed to read memory")
}

pub async fn get_history(
    pool: &SqlitePool,
    key: &str,
    limit: Option<i64>,
) -> Result<Vec<MemoryVersionRow>> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
    sqlx::query_as::<_, MemoryVersionRow>(
        "SELECT * FROM memory_versions
         WHERE key = $1
         ORDER BY version DESC
         LIMIT $2",
    )
    .bind(key)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("failed to get memory history")
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum MemoryListResult {
    Full(PaginatedResponse<MemoryRow>),
    Summary(PaginatedResponse<MemorySummaryRow>),
}

pub async fn search(
    pool: &SqlitePool,
    query: &str,
    params: &ListParams<'_>,
) -> Result<MemoryListResult> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE);
    let (cursor_ts, cursor_key) = parse_cursor(params.cursor);
    let memory_type = params.memory_type.as_ref();

    if params.detail.is_summary() {
        let rows = sqlx::query_as::<_, MemorySummaryRow>(
            "SELECT m.key, SUBSTR(m.content, 1, 100) as content_preview, m.memory_type, m.project, m.tags, m.version, m.updated_at
             FROM memories m
             JOIN memories_fts fts ON m.rowid = fts.rowid
             WHERE memories_fts MATCH $1
               AND ($2 IS NULL OR m.memory_type = $2)
               AND ($3 IS NULL OR m.project = $3)
               AND ($4 IS NULL OR EXISTS (SELECT 1 FROM json_each(m.tags) WHERE value = $4))
               AND ($5 IS NULL OR m.updated_at < $5 OR (m.updated_at = $5 AND m.key > $6))
             ORDER BY m.updated_at DESC, m.key ASC
             LIMIT $7",
        )
        .bind(query)
        .bind(memory_type.map(|t| t.as_str()))
        .bind(params.project)
        .bind(params.tag)
        .bind(&cursor_ts)
        .bind(&cursor_key)
        .bind(limit + 1)
        .fetch_all(pool)
        .await
        .context("failed to search memories")?;
        Ok(MemoryListResult::Summary(paginate(rows, limit)))
    } else {
        let rows = sqlx::query_as::<_, MemoryRow>(
            "SELECT m.* FROM memories m
             JOIN memories_fts fts ON m.rowid = fts.rowid
             WHERE memories_fts MATCH $1
               AND ($2 IS NULL OR m.memory_type = $2)
               AND ($3 IS NULL OR m.project = $3)
               AND ($4 IS NULL OR EXISTS (SELECT 1 FROM json_each(m.tags) WHERE value = $4))
               AND ($5 IS NULL OR m.updated_at < $5 OR (m.updated_at = $5 AND m.key > $6))
             ORDER BY m.updated_at DESC, m.key ASC
             LIMIT $7",
        )
        .bind(query)
        .bind(memory_type.map(|t| t.as_str()))
        .bind(params.project)
        .bind(params.tag)
        .bind(&cursor_ts)
        .bind(&cursor_key)
        .bind(limit + 1)
        .fetch_all(pool)
        .await
        .context("failed to search memories")?;
        Ok(MemoryListResult::Full(paginate(rows, limit)))
    }
}

pub async fn list(pool: &SqlitePool, params: &ListParams<'_>) -> Result<MemoryListResult> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE);
    let (cursor_ts, cursor_key) = parse_cursor(params.cursor);
    let memory_type = params.memory_type.as_ref();

    if params.detail.is_summary() {
        let rows = sqlx::query_as::<_, MemorySummaryRow>(
            "SELECT key, SUBSTR(content, 1, 100) as content_preview, memory_type, project, tags, version, updated_at
             FROM memories
             WHERE ($1 IS NULL OR memory_type = $1)
               AND ($2 IS NULL OR project = $2)
               AND ($3 IS NULL OR EXISTS (SELECT 1 FROM json_each(tags) WHERE value = $3))
               AND ($4 IS NULL OR updated_at < $4 OR (updated_at = $4 AND key > $5))
             ORDER BY updated_at DESC, key ASC
             LIMIT $6",
        )
        .bind(memory_type.map(|t| t.as_str()))
        .bind(params.project)
        .bind(params.tag)
        .bind(&cursor_ts)
        .bind(&cursor_key)
        .bind(limit + 1)
        .fetch_all(pool)
        .await
        .context("failed to list memories")?;
        Ok(MemoryListResult::Summary(paginate(rows, limit)))
    } else {
        let rows = sqlx::query_as::<_, MemoryRow>(
            "SELECT * FROM memories
             WHERE ($1 IS NULL OR memory_type = $1)
               AND ($2 IS NULL OR project = $2)
               AND ($3 IS NULL OR EXISTS (SELECT 1 FROM json_each(tags) WHERE value = $3))
               AND ($4 IS NULL OR updated_at < $4 OR (updated_at = $4 AND key > $5))
             ORDER BY updated_at DESC, key ASC
             LIMIT $6",
        )
        .bind(memory_type.map(|t| t.as_str()))
        .bind(params.project)
        .bind(params.tag)
        .bind(&cursor_ts)
        .bind(&cursor_key)
        .bind(limit + 1)
        .fetch_all(pool)
        .await
        .context("failed to list memories")?;
        Ok(MemoryListResult::Full(paginate(rows, limit)))
    }
}

pub async fn delete(pool: &SqlitePool, key: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM memories WHERE key = $1")
        .bind(key)
        .execute(pool)
        .await
        .context("failed to delete memory")?;
    Ok(result.rows_affected() > 0)
}
