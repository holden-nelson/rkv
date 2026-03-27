mod config;
mod context;

use crate::{config::load_config, context::context::NodeContext};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config("example_cluster_config.toml")?;
    let context = NodeContext::from_config(config, "node2");

    let _ = dbg!(context);

    Ok(())
}
