use anyhow::Result;

use crate::bootstrap::bootstrap_node;

mod bootstrap;
mod config;
mod context;
mod core;
mod tasks;

#[tokio::main]
async fn main() -> Result<()> {
    bootstrap_node().await?;

    Ok(())
}
