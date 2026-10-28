mod app;
mod config;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use hma_core::db::Db;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

#[derive(Parser)]
#[command(version, about = "Explore Magic cards to build themed Commander decks")]
struct Cli {
    /// TOML config file, defaults to $XDG_CONFIG_HOME/hii-magic-ation/config.toml
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the API and the web UI (default)
    Serve,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref()).context("invalid configuration")?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(config).await,
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    let db = Db::open(&config.data_dir)
        .with_context(|| format!("cannot open databases in {}", config.data_dir.display()))?;
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .with_context(|| format!("cannot bind {}", config.bind))?;
    tracing::info!(addr = %config.bind, data_dir = %config.data_dir.display(), "listening");
    axum::serve(listener, app::router(db))
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
