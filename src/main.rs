mod artifact;
mod db;
mod event;
mod memory;
mod schema;
mod server;
mod stats;

use std::future::Future;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::EnvFilter;

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
    #[arg(long, env = "MEMENTO_DB_PATH")]
    db_path: Option<PathBuf>,

    /// Transport protocol to use
    #[arg(long, default_value = "stdio")]
    transport: Transport,

    /// Port for HTTP transport
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Bind address for HTTP transport
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
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

    let (shutdown_token, shutdown_fut) = shutdown_signal();

    match cli.transport {
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
            let bind_addr = format!("{}:{}", cli.host, cli.port);
            info!(addr = %bind_addr, "starting HTTP transport");

            let config = StreamableHttpServerConfig::default()
                .with_cancellation_token(shutdown_token.child_token());

            let service = StreamableHttpService::new(
                move || Ok(MementoServer::new(pool.clone())),
                LocalSessionManager::default().into(),
                config,
            );

            let router = axum::Router::new().nest_service("/mcp", service);
            let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown_fut)
                .await?;
        }
    }

    info!("memento shutdown complete");
    Ok(())
}

/// Returns a cancellation token and a future that resolves on SIGINT or SIGTERM.
///
/// The token is cancelled when either signal fires, cascading shutdown to all
/// listeners that hold a clone or child token.
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
