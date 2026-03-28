use anyhow::Result;

use crate::bootstrap::bootstrap_node;

mod bootstrap;
mod config;
mod context;
mod core;

fn main() -> Result<()> {
    bootstrap_node()?;

    Ok(())
}
