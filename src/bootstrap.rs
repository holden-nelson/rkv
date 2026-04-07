use std::fs::create_dir_all;

use anyhow::Result;

use crate::{
    config::load_config,
    context::NodeContext,
    core::{run, state},
};

pub async fn bootstrap_node(node_id: &str) -> Result<()> {
    let config = load_config("example_cluster_config.toml")?;
    let context = NodeContext::from_config(config, node_id)?;

    // create base dir
    create_dir_all(&context.persistence.base_dir)?;

    let node_state = state::NodeState::new();
    node_state.persistent_state.write_to_disk(&context)?;

    run::run(context, node_state).await?;

    Ok(())
}
