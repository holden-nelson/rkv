use anyhow::Result;

mod bootstrap;
mod config;
mod context;
mod core;
mod tasks;

#[tokio::main]
async fn main() -> Result<()> {
    tokio::try_join!(
        bootstrap::bootstrap_node("node1"),
        bootstrap::bootstrap_node("node2"),
        bootstrap::bootstrap_node("node3"),
    )?;
    Ok(())
}
