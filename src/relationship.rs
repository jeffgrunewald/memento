use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RelationshipRow {
    pub source_type: String,
    pub source_key: String,
    pub target_type: String,
    pub target_key: String,
    pub relation: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Related {
    pub outgoing: Vec<RelationshipRow>,
    pub incoming: Vec<RelationshipRow>,
}

pub async fn link(
    pool: &SqlitePool,
    source_type: &str,
    source_key: &str,
    target_type: &str,
    target_key: &str,
    relation: &str,
) -> Result<RelationshipRow> {
    sqlx::query_as::<_, RelationshipRow>(
        "INSERT INTO relationships (source_type, source_key, target_type, target_key, relation)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT DO NOTHING
         RETURNING *",
    )
    .bind(source_type)
    .bind(source_key)
    .bind(target_type)
    .bind(target_key)
    .bind(relation)
    .fetch_optional(pool)
    .await
    .context("failed to create link")?
    .context("link already exists")
}

pub async fn unlink(
    pool: &SqlitePool,
    source_type: &str,
    source_key: &str,
    target_type: &str,
    target_key: &str,
    relation: Option<&str>,
) -> Result<u64> {
    let result = if let Some(relation) = relation {
        sqlx::query(
            "DELETE FROM relationships
             WHERE source_type = $1 AND source_key = $2
               AND target_type = $3 AND target_key = $4
               AND relation = $5",
        )
        .bind(source_type)
        .bind(source_key)
        .bind(target_type)
        .bind(target_key)
        .bind(relation)
        .execute(pool)
        .await
    } else {
        sqlx::query(
            "DELETE FROM relationships
             WHERE source_type = $1 AND source_key = $2
               AND target_type = $3 AND target_key = $4",
        )
        .bind(source_type)
        .bind(source_key)
        .bind(target_type)
        .bind(target_key)
        .execute(pool)
        .await
    }
    .context("failed to remove link")?;

    Ok(result.rows_affected())
}

pub async fn get_related(
    pool: &SqlitePool,
    entity_type: &str,
    key: &str,
    relation: Option<&str>,
) -> Result<Related> {
    let outgoing = sqlx::query_as::<_, RelationshipRow>(
        "SELECT * FROM relationships
         WHERE source_type = $1 AND source_key = $2
           AND ($3 IS NULL OR relation = $3)
         ORDER BY created_at DESC",
    )
    .bind(entity_type)
    .bind(key)
    .bind(relation)
    .fetch_all(pool)
    .await
    .context("failed to get outgoing relationships")?;

    let incoming = sqlx::query_as::<_, RelationshipRow>(
        "SELECT * FROM relationships
         WHERE target_type = $1 AND target_key = $2
           AND ($3 IS NULL OR relation = $3)
         ORDER BY created_at DESC",
    )
    .bind(entity_type)
    .bind(key)
    .bind(relation)
    .fetch_all(pool)
    .await
    .context("failed to get incoming relationships")?;

    Ok(Related { outgoing, incoming })
}
