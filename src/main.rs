mod artifact;
mod db;
mod display;
mod event;
mod import;
mod memory;
mod relationship;
mod schema;
mod server;
mod stats;
mod util;

use std::future::Future;
use std::path::PathBuf;

use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use clap::{Parser, Subcommand, ValueEnum};
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::schema::MemoryType;
use crate::server::MementoServer;

#[derive(Debug, Clone, ValueEnum)]
enum Transport {
    Stdio,
    Http,
}

#[derive(Parser)]
#[command(
    name = "memento",
    about = "MCP memory server for multi-agent coordination"
)]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, global = true, env = "MEMENTO_DB_PATH")]
    db_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the MCP server (default if no subcommand given)
    Serve {
        /// Transport protocol to use
        #[arg(long, default_value = "stdio")]
        transport: Transport,
        /// Port for HTTP transport
        #[arg(long, default_value = "8080")]
        port: u16,
        /// Bind address for HTTP transport
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    /// Import memories from agent framework memory/rules files
    Import {
        /// Framework to import from (default: all)
        #[arg(long, default_value = "all")]
        source: import::Source,
        /// Workspace root to scan for project-level files
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Query the database directly from the command line
    Query {
        /// Output raw JSON instead of human-readable format
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        action: QueryAction,
    },
}

#[derive(Subcommand)]
enum QueryAction {
    /// Show summary statistics
    Stats,
    /// List memories with optional filters
    Memories {
        /// Filter by memory type
        #[arg(long)]
        r#type: Option<String>,
        /// Filter by project
        #[arg(long)]
        project: Option<String>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: i64,
    },
    /// Search memories by text
    Search {
        /// Search query
        query: String,
        /// Filter by memory type
        #[arg(long)]
        r#type: Option<String>,
        /// Filter by project
        #[arg(long)]
        project: Option<String>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: i64,
    },
    /// List artifacts
    Artifacts {
        /// Filter by artifact type
        #[arg(long)]
        r#type: Option<String>,
        /// Filter by project
        #[arg(long)]
        project: Option<String>,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: i64,
    },
    /// List recent events
    Events {
        /// Show events after this ID
        #[arg(long)]
        after: Option<i64>,
        /// Filter by event type
        #[arg(long)]
        r#type: Option<String>,
        /// Filter by project
        #[arg(long)]
        project: Option<String>,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: i64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();
    let db_path = cli.db_path.unwrap_or_else(db::default_db_path);
    let pool = db::connect(&db_path).await?;

    match cli.command.unwrap_or(Command::Serve {
        transport: Transport::Stdio,
        port: 8080,
        host: "127.0.0.1".to_string(),
    }) {
        Command::Serve {
            transport,
            port,
            host,
        } => run_server(pool, transport, &host, port).await?,
        Command::Import { source, workspace } => run_import(pool, source, workspace).await?,
        Command::Query { json, action } => run_query(pool, action, json).await?,
    }

    Ok(())
}

async fn run_server(
    pool: sqlx::SqlitePool,
    transport: Transport,
    host: &str,
    port: u16,
) -> Result<()> {
    let (shutdown_token, shutdown_fut) = shutdown_signal();

    match transport {
        Transport::Stdio => {
            info!("starting stdio transport");
            let service = MementoServer::new(pool)
                .serve(rmcp::transport::stdio())
                .await?;

            tokio::select! {
                result = service.waiting() => { result?; }
                () = shutdown_fut => {}
            }
        }
        Transport::Http => {
            let bind_addr = format!("{host}:{port}");
            info!(addr = %bind_addr, "starting HTTP transport");

            let config = StreamableHttpServerConfig::default()
                .with_cancellation_token(shutdown_token.child_token());

            let health_pool = pool.clone();

            let service = StreamableHttpService::new(
                move || Ok(MementoServer::new(pool.clone())),
                LocalSessionManager::default().into(),
                config,
            );

            let router = axum::Router::new()
                .route("/health", get(health_check))
                .nest_service("/mcp", service)
                .with_state(health_pool);
            let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown_fut)
                .await?;
        }
    }

    info!("memento shutdown complete");
    Ok(())
}

/// Liveness probe: confirms the process can reach and query the SQLite pool.
async fn health_check(State(pool): State<sqlx::SqlitePool>) -> StatusCode {
    match sqlx::query("SELECT 1").fetch_one(&pool).await {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            tracing::warn!(error = %e, "health check failed");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

async fn run_import(
    pool: sqlx::SqlitePool,
    source: import::Source,
    workspace: Option<PathBuf>,
) -> Result<()> {
    let report = import::import_files(&pool, source, workspace.as_deref()).await?;

    if report.imported.is_empty() && report.skipped.is_empty() && report.errors.is_empty() {
        println!("No memory files found.");
        return Ok(());
    }

    if !report.imported.is_empty() {
        println!("Imported {} memories:", report.imported.len());
        for item in &report.imported {
            println!(
                "  + {} ({}{})",
                item.key,
                item.memory_type,
                item.project
                    .as_ref()
                    .map_or(String::new(), |p| format!(", project={p}"))
            );
        }
        println!();
    }

    if !report.skipped.is_empty() {
        println!("Skipped {} (already exist):", report.skipped.len());
        for item in &report.skipped {
            println!("  - {} ({})", item.key, item.source);
        }
        println!();
    }

    if !report.errors.is_empty() {
        println!("Errors ({}):", report.errors.len());
        for item in &report.errors {
            println!("  ! {}: {}", item.source, item.error);
        }
        println!();
    }

    println!(
        "Done: {} imported, {} skipped, {} errors",
        report.imported.len(),
        report.skipped.len(),
        report.errors.len()
    );

    Ok(())
}

fn print_memory_result(result: &memory::MemoryListResult, json: bool) -> Result<()> {
    match result {
        memory::MemoryListResult::Full(page) => {
            if json {
                println!("{}", rmcp::serde_json::to_string_pretty(page)?);
            } else {
                print!("{}", display::format_memories(page));
            }
        }
        memory::MemoryListResult::Summary(page) => {
            if json {
                println!("{}", rmcp::serde_json::to_string_pretty(page)?);
            } else {
                print!("{}", display::format_memory_summaries(page));
            }
        }
    }
    Ok(())
}

async fn run_query(pool: sqlx::SqlitePool, action: QueryAction, json: bool) -> Result<()> {
    match action {
        QueryAction::Stats => {
            let s = stats::get_stats(&pool).await?;
            println!("{}", rmcp::serde_json::to_string_pretty(&s)?);
        }
        QueryAction::Memories {
            r#type,
            project,
            tag,
            limit,
        } => {
            let params = memory::ListParams {
                memory_type: r#type.as_deref().and_then(MemoryType::parse),
                project: project.as_deref(),
                tag: tag.as_deref(),
                cursor: None,
                limit: Some(limit),
                detail: crate::schema::ContentDetail::Full,
            };
            let result = memory::list(&pool, &params).await?;
            print_memory_result(&result, json)?;
        }
        QueryAction::Search {
            query,
            r#type,
            project,
            tag,
            limit,
        } => {
            let params = memory::ListParams {
                memory_type: r#type.as_deref().and_then(MemoryType::parse),
                project: project.as_deref(),
                tag: tag.as_deref(),
                cursor: None,
                limit: Some(limit),
                detail: crate::schema::ContentDetail::Full,
            };
            let result = memory::search(&pool, &query, &params).await?;
            print_memory_result(&result, json)?;
        }
        QueryAction::Artifacts {
            r#type,
            project,
            limit,
        } => {
            let result = artifact::list(
                &pool,
                r#type.as_deref(),
                project.as_deref(),
                None,
                Some(limit),
                crate::schema::ContentDetail::Full,
            )
            .await?;
            match result {
                artifact::ArtifactListResult::Full(ref page) => {
                    if json {
                        println!("{}", rmcp::serde_json::to_string_pretty(page)?);
                    } else {
                        print!("{}", display::format_artifacts(page));
                    }
                }
                artifact::ArtifactListResult::Summary(ref page) => {
                    if json {
                        println!("{}", rmcp::serde_json::to_string_pretty(page)?);
                    } else {
                        print!("{}", display::format_artifact_summaries(page));
                    }
                }
            }
        }
        QueryAction::Events {
            after,
            r#type,
            project,
            limit,
        } => {
            let page = event::read_since(
                &pool,
                after,
                r#type.as_deref(),
                project.as_deref(),
                Some(limit),
            )
            .await?;
            if json {
                println!("{}", rmcp::serde_json::to_string_pretty(&page)?);
            } else {
                print!("{}", display::format_events(&page));
            }
        }
    }
    Ok(())
}

fn shutdown_signal() -> (CancellationToken, impl Future<Output = ()>) {
    let token = CancellationToken::new();
    let cancel = token.clone();

    let fut = async move {
        let ctrl_c = async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install ctrl+c handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            () = ctrl_c => {
                info!("received SIGINT, initiating graceful shutdown");
            }
            () = terminate => {
                info!("received SIGTERM, initiating graceful shutdown");
            }
        }

        cancel.cancel();
    };

    (token, fut)
}
