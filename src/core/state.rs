use std::{collections::HashMap, io};

use serde::{Deserialize, Serialize};

use crate::{context::NodeContext, core::storage::atomic_write};

#[derive(Serialize, Deserialize)]
pub struct PersistentState {
    pub current_term: u64,
    pub voted_for: Option<String>,
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
    pub commit_index: u64,
    pub last_applied: u64,
    pub last_logged_index: u64,
    pub last_logged_term: u64,
}

impl VolatileState {
    pub fn new() -> Self {
        VolatileState {
            commit_index: 0,
            last_applied: 0,
            last_logged_index: 0,
            last_logged_term: 0,
        }
    }
}

pub struct LeaderState {
    pub next_index: HashMap<String, u64>,
    pub match_index: HashMap<String, u64>,
}

impl LeaderState {
    pub fn new() -> Self {
        LeaderState {
            next_index: HashMap::new(),
            match_index: HashMap::new(),
        }
    }
}

pub struct NodeState {
    pub persistent_state: PersistentState,
    pub volatile_state: VolatileState,
    pub leader_state: Option<LeaderState>,
}

impl NodeState {
    pub fn new() -> Self {
        NodeState {
            persistent_state: PersistentState::new(),
            volatile_state: VolatileState::new(),
            leader_state: None,
        }
    }
}
