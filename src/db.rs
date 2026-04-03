use std::path::PathBuf;

use anyhow::{Context, Result};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tracing::info;

const DEFAULT_DB_DIR: &str = ".config/claude-memory";
const DEFAULT_DB_NAME: &str = "memory.db";

pub fn default_db_path() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    PathBuf::from(home)
        .join(DEFAULT_DB_DIR)
        .join(DEFAULT_DB_NAME)
}

pub async fn connect(db_path: &std::path::Path) -> Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).context("failed to create database directory")?;
    }

    let url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .context("failed to connect to SQLite")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    info!(path = %db_path.display(), "database ready");

    Ok(pool)
}
