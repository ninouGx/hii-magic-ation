mod app;
mod catalog_api;
mod config;
mod sync_job;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use hma_core::db::Db;
use hma_ingest::scryfall::{CardSource, ScryfallClient};
use hma_ingest::sync::SyncPlan;
use hma_ingest::verify::{SCRYFALL_COMMANDER_QUERY, diff_commanders};
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
    /// Download changed sources and rebuild the catalog
    Sync {
        /// Sync one source only
        #[arg(long, value_enum)]
        only: Option<Source>,
        /// Rebuild the catalog even when no source changed
        #[arg(long)]
        rebuild: bool,
    },
    /// Compare the catalog with Scryfall
    Verify {
        #[command(subcommand)]
        target: VerifyTarget,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Source {
    Scryfall,
    Edhrec,
}

#[derive(Subcommand)]
enum VerifyTarget {
    /// Diff commander eligibility against the Scryfall search `is:commander legal:commander`
    Commanders,
}

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref()).context("invalid configuration")?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(config).await.map(|()| ExitCode::SUCCESS),
        Command::Sync { only, rebuild } => sync(&config, only, rebuild).await,
        Command::Verify {
            target: VerifyTarget::Commanders,
        } => verify_commanders(&config).await,
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    let db = Db::open(&config.data_dir)
        .with_context(|| format!("cannot open databases in {}", config.data_dir.display()))?;
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .with_context(|| format!("cannot bind {}", config.bind))?;
    tracing::info!(addr = %config.bind, data_dir = %config.data_dir.display(), "listening");
    axum::serve(listener, app::router(app::AppState::new(db)))
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}

async fn sync(config: &Config, only: Option<Source>, rebuild: bool) -> anyhow::Result<ExitCode> {
    let plan = SyncPlan {
        scryfall: !matches!(only, Some(Source::Edhrec)),
        edhrec: !matches!(only, Some(Source::Scryfall)),
        rebuild,
    };
    std::fs::create_dir_all(&config.data_dir)?;
    let previous = sync_job::latest_catalog_counts(&config.data_dir);
    let report = sync_job::run(&config.data_dir, previous, plan).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);

    if report.catalog.is_some() {
        match notify_server(config).await {
            Ok(()) => println!("The running server now uses the new catalog."),
            Err(e) => {
                tracing::debug!(error = %e, "server not reachable");
                println!(
                    "No server reachable at {}, the new catalog loads at next start.",
                    config.bind
                );
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

async fn notify_server(config: &Config) -> anyhow::Result<()> {
    reqwest::Client::new()
        .post(format!("http://{}/api/catalog/reload", config.bind))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn verify_commanders(config: &Config) -> anyhow::Result<ExitCode> {
    let scryfall = ScryfallClient::new(sync_job::USER_AGENT)?
        .search_oracle_ids(SCRYFALL_COMMANDER_QUERY)
        .await?;
    let conn = sync_job::open_latest_catalog(&config.data_dir)?;
    let diff = diff_commanders(&conn, "main", &scryfall)?;
    println!("{}", serde_json::to_string_pretty(&diff)?);
    Ok(if diff.is_exact() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
