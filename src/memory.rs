use anyhow::{Context, Result};
use sqlx::SqlitePool;

use crate::schema::{DEFAULT_PAGE_SIZE, MemoryRow, MemoryType, PaginatedResponse, paginate, parse_cursor};

pub async fn write(
    pool: &SqlitePool,
    key: &str,
    content: &str,
    memory_type: MemoryType,
    project: Option<&str>,
    tags: &str,
) -> Result<MemoryRow> {
    sqlx::query_as::<_, MemoryRow>(
        "INSERT INTO memories (key, content, memory_type, project, tags)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT(key) DO UPDATE SET
             content = excluded.content,
             memory_type = excluded.memory_type,
             project = excluded.project,
             tags = excluded.tags,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         RETURNING *",
    )
    .bind(key)
    .bind(content)
    .bind(memory_type.as_str())
    .bind(project)
    .bind(tags)
    .fetch_one(pool)
    .await
    .context("failed to write memory")
}

pub async fn read(pool: &SqlitePool, key: &str) -> Result<Option<MemoryRow>> {
    sqlx::query_as::<_, MemoryRow>("SELECT * FROM memories WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .context("failed to read memory")
}

pub async fn search(
    pool: &SqlitePool,
    query: &str,
    memory_type: Option<MemoryType>,
    project: Option<&str>,
    tag: Option<&str>,
    cursor: Option<&str>,
    limit: Option<i64>,
) -> Result<PaginatedResponse<MemoryRow>> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
    let (cursor_ts, cursor_key) = parse_cursor(cursor);

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
    .bind(project)
    .bind(tag)
    .bind(&cursor_ts)
    .bind(&cursor_key)
    .bind(limit + 1)
    .fetch_all(pool)
    .await
    .context("failed to search memories")?;

    Ok(paginate(rows, limit))
}

pub async fn list(
    pool: &SqlitePool,
    memory_type: Option<MemoryType>,
    project: Option<&str>,
    tag: Option<&str>,
    cursor: Option<&str>,
    limit: Option<i64>,
) -> Result<PaginatedResponse<MemoryRow>> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
    let (cursor_ts, cursor_key) = parse_cursor(cursor);

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
    .bind(project)
    .bind(tag)
    .bind(&cursor_ts)
    .bind(&cursor_key)
    .bind(limit + 1)
    .fetch_all(pool)
    .await
    .context("failed to list memories")?;

    Ok(paginate(rows, limit))
}

pub async fn delete(pool: &SqlitePool, key: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM memories WHERE key = $1")
        .bind(key)
        .execute(pool)
        .await
        .context("failed to delete memory")?;
    Ok(result.rows_affected() > 0)
}

