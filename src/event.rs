use anyhow::{Context, Result};
use sqlx::SqlitePool;

use crate::schema::{DEFAULT_PAGE_SIZE, EventRow, PaginatedResponse, paginate};

pub async fn append(
    pool: &SqlitePool,
    source_agent: &str,
    event_type: &str,
    payload: &str,
    project: Option<&str>,
) -> Result<EventRow> {
    sqlx::query_as::<_, EventRow>(
        "INSERT INTO events (source_agent, event_type, payload, project)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(source_agent)
    .bind(event_type)
    .bind(payload)
    .bind(project)
    .fetch_one(pool)
    .await
    .context("failed to append event")
}

pub async fn read_since(
    pool: &SqlitePool,
    after_id: Option<i64>,
    event_type: Option<&str>,
    project: Option<&str>,
    limit: Option<i64>,
) -> Result<PaginatedResponse<EventRow>> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
    let after_id = after_id.unwrap_or(0);

    let rows = sqlx::query_as::<_, EventRow>(
        "SELECT * FROM events
         WHERE id > $1
           AND ($2 IS NULL OR event_type = $2)
           AND ($3 IS NULL OR project = $3)
         ORDER BY id ASC
         LIMIT $4",
    )
    .bind(after_id)
    .bind(event_type)
    .bind(project)
    .bind(limit + 1)
    .fetch_all(pool)
    .await
    .context("failed to read events")?;

    Ok(paginate(rows, limit))
}
