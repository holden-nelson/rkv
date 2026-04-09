use std::fs::create_dir_all;

use anyhow::Result;

use crate::{config::load_config, context::NodeContext, core::run};

pub async fn bootstrap_node(node_id: &str) -> Result<()> {
    let config = load_config("example_cluster_config.toml")?;
    let context = NodeContext::from_config(config, node_id)?;

    // create base dir
    create_dir_all(&context.persistence.base_dir)?;

    run::run(context).await?;

    Ok(())
}
