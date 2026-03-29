use std::fs::create_dir_all;

use anyhow::Result;

use crate::{
    config::load_config,
    context::NodeContext,
    core::{run, state},
};

pub async fn bootstrap_node() -> Result<()> {
    let config = load_config("example_cluster_config.toml")?;
    let context = NodeContext::from_config(config, "node2")?;

    // create base dir
    create_dir_all(&context.persistence.base_dir)?;

    let persistent_state = state::PersistentState::new();
    persistent_state.write_to_disk(&context)?;

    run::run(&context).await;

    Ok(())
}
