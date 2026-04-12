use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod bootstrap;
mod config;
mod context;
mod core;
mod tasks;

#[derive(Debug, Parser)]
#[command(name = "rkv")]
struct Cli {
    /// Enable verbose debug logging.
    #[arg(long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let default_filter = if cli.debug { "rkv=debug" } else { "rkv=info" };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    tokio::try_join!(
        bootstrap::bootstrap_node("node1"),
        bootstrap::bootstrap_node("node2"),
        bootstrap::bootstrap_node("node3"),
    )?;
    Ok(())
}
