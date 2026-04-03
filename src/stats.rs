use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TypeCount {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub memories: StatsGroup,
    pub artifacts: StatsGroup,
    pub events: StatsGroup,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsGroup {
    pub total: i64,
    pub by_type: Vec<TypeCount>,
    pub by_project: Vec<TypeCount>,
}

pub async fn get_stats(pool: &SqlitePool) -> Result<Stats> {
    let memories = stats_group(
        pool,
        "SELECT COUNT(*) as total FROM memories",
        "SELECT memory_type as label, COUNT(*) as count FROM memories GROUP BY memory_type ORDER BY count DESC",
        "SELECT COALESCE(project, '(global)') as label, COUNT(*) as count FROM memories GROUP BY project ORDER BY count DESC",
    ).await.context("failed to get memory stats")?;

    let artifacts = stats_group(
        pool,
        "SELECT COUNT(*) as total FROM artifacts WHERE expires_at IS NULL OR expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        "SELECT artifact_type as label, COUNT(*) as count FROM artifacts WHERE expires_at IS NULL OR expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') GROUP BY artifact_type ORDER BY count DESC",
        "SELECT COALESCE(project, '(global)') as label, COUNT(*) as count FROM artifacts WHERE expires_at IS NULL OR expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now') GROUP BY project ORDER BY count DESC",
    ).await.context("failed to get artifact stats")?;

    let events = stats_group(
        pool,
        "SELECT COUNT(*) as total FROM events",
        "SELECT event_type as label, COUNT(*) as count FROM events GROUP BY event_type ORDER BY count DESC",
        "SELECT COALESCE(project, '(global)') as label, COUNT(*) as count FROM events GROUP BY project ORDER BY count DESC",
    ).await.context("failed to get event stats")?;

    Ok(Stats {
        memories,
        artifacts,
        events,
    })
}

async fn stats_group(
    pool: &SqlitePool,
    total_sql: &str,
    by_type_sql: &str,
    by_project_sql: &str,
) -> Result<StatsGroup, sqlx::Error> {
    let total: (i64,) = sqlx::query_as(total_sql).fetch_one(pool).await?;
    let by_type = sqlx::query_as::<_, TypeCount>(by_type_sql)
        .fetch_all(pool)
        .await?;
    let by_project = sqlx::query_as::<_, TypeCount>(by_project_sql)
        .fetch_all(pool)
        .await?;

    Ok(StatsGroup {
        total: total.0,
        by_type,
        by_project,
    })
}
