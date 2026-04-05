use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::SqlitePool;
use time::OffsetDateTime;

use crate::schema::{
    ArtifactRow, ArtifactSummaryRow, ContentDetail, DEFAULT_PAGE_SIZE, PaginatedResponse, paginate,
    parse_cursor,
};

pub async fn write(
    pool: &SqlitePool,
    key: &str,
    content: &str,
    artifact_type: &str,
    project: Option<&str>,
    source_agent: Option<&str>,
    expires_at: Option<OffsetDateTime>,
) -> Result<ArtifactRow> {
    sqlx::query_as::<_, ArtifactRow>(
        "INSERT INTO artifacts (key, content, artifact_type, project, source_agent, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT(key) DO UPDATE SET
             content = excluded.content,
             artifact_type = excluded.artifact_type,
             project = excluded.project,
             source_agent = excluded.source_agent,
             expires_at = excluded.expires_at
         RETURNING *",
    )
    .bind(key)
    .bind(content)
    .bind(artifact_type)
    .bind(project)
    .bind(source_agent)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .context("failed to write artifact")
}

pub async fn read(pool: &SqlitePool, key: &str) -> Result<Option<ArtifactRow>> {
    sqlx::query_as::<_, ArtifactRow>(
        "SELECT * FROM artifacts WHERE key = $1 AND (expires_at IS NULL OR expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .bind(key)
    .fetch_optional(pool)
    .await
    .context("failed to read artifact")
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ArtifactListResult {
    Full(PaginatedResponse<ArtifactRow>),
    Summary(PaginatedResponse<ArtifactSummaryRow>),
}

pub async fn list(
    pool: &SqlitePool,
    artifact_type: Option<&str>,
    project: Option<&str>,
    cursor: Option<&str>,
    limit: Option<i64>,
    detail: ContentDetail,
) -> Result<ArtifactListResult> {
    cleanup_expired(pool).await?;

    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
    let (cursor_ts, cursor_key) = parse_cursor(cursor);

    if detail.is_summary() {
        let rows = sqlx::query_as::<_, ArtifactSummaryRow>(
            "SELECT key, SUBSTR(content, 1, 100) as content_preview, artifact_type, project, source_agent, created_at
             FROM artifacts
             WHERE (expires_at IS NULL OR expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
               AND ($1 IS NULL OR artifact_type = $1)
               AND ($2 IS NULL OR project = $2)
               AND ($3 IS NULL OR created_at < $3 OR (created_at = $3 AND key > $4))
             ORDER BY created_at DESC, key ASC
             LIMIT $5",
        )
        .bind(artifact_type)
        .bind(project)
        .bind(&cursor_ts)
        .bind(&cursor_key)
        .bind(limit + 1)
        .fetch_all(pool)
        .await
        .context("failed to list artifacts")?;
        Ok(ArtifactListResult::Summary(paginate(rows, limit)))
    } else {
        let rows = sqlx::query_as::<_, ArtifactRow>(
            "SELECT * FROM artifacts
             WHERE (expires_at IS NULL OR expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
               AND ($1 IS NULL OR artifact_type = $1)
               AND ($2 IS NULL OR project = $2)
               AND ($3 IS NULL OR created_at < $3 OR (created_at = $3 AND key > $4))
             ORDER BY created_at DESC, key ASC
             LIMIT $5",
        )
        .bind(artifact_type)
        .bind(project)
        .bind(&cursor_ts)
        .bind(&cursor_key)
        .bind(limit + 1)
        .fetch_all(pool)
        .await
        .context("failed to list artifacts")?;
        Ok(ArtifactListResult::Full(paginate(rows, limit)))
    }
}

async fn cleanup_expired(pool: &SqlitePool) -> Result<()> {
    let result = sqlx::query(
        "DELETE FROM artifacts WHERE expires_at IS NOT NULL AND expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .execute(pool)
    .await
    .context("failed to cleanup expired artifacts")?;

    if result.rows_affected() > 0 {
        tracing::debug!(count = result.rows_affected(), "cleaned up expired artifacts");
    }
    Ok(())
}
