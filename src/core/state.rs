use std::{collections::HashMap, io};

use serde::{Deserialize, Serialize};

use crate::{context::NodeContext, core::storage::atomic_write};

#[derive(Serialize, Deserialize)]
pub struct PersistentState {
    current_term: u64,
    voted_for: Option<String>,
}

impl PersistentState {
    pub fn new() -> Self {
        PersistentState {
            current_term: 0,
            voted_for: None,
        }
    }

    pub fn write_to_disk(&self, ctx: &NodeContext) -> io::Result<()> {
        let bytes = serde_json::to_vec(&self)?;
        let path = ctx.persistence.base_dir.join("state.json");
        atomic_write(&path, &bytes)
    }
}

pub struct VolatileState {
    commit_index: u64,
    last_applied: u64,
}

pub struct LeaderState {
    next_index: HashMap<String, u64>,
    match_index: HashMap<String, u64>,
}
