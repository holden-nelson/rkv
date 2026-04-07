use std::{collections::HashMap, io};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{context::NodeContext, core::storage::atomic_write};

#[derive(Serialize, Deserialize)]
struct PersistentState {
    current_term: u64,
    voted_for: Option<String>,
}

impl PersistentState {
    fn new(ctx: &NodeContext) -> Result<Self> {
        let state = PersistentState {
            current_term: 0,
            voted_for: None,
        };

        state.write_to_disk(ctx)?;

        Ok(state)
    }

    fn write_to_disk(&self, ctx: &NodeContext) -> io::Result<()> {
        let bytes = serde_json::to_vec(&self)?;
        let path = ctx.persistence.base_dir.join("state.json");
        atomic_write(&path, &bytes)
    }
}

struct VolatileState {
    role: NodeRole,
    commit_index: u64,
    last_applied: u64,
    current_election_timeout_ms: u32,
    last_logged_index: u64,
    last_logged_term: u64,
}

impl VolatileState {
    fn new(ctx: &NodeContext) -> Self {
        VolatileState {
            role: NodeRole::Follower,
            current_election_timeout_ms: generate_election_timeout_ms(
                ctx.election_timeout_min,
                ctx.election_timeout_max,
            ),
            commit_index: 0,
            last_applied: 0,
            last_logged_index: 0,
            last_logged_term: 0,
        }
    }
}

#[derive(Debug)]
enum NodeRole {
    Leader,
    Candidate,
    Follower,
}

pub struct LeaderState {
    next_index: HashMap<String, u64>,
    match_index: HashMap<String, u64>,
}

impl LeaderState {
    fn new() -> Self {
        LeaderState {
            next_index: HashMap::new(),
            match_index: HashMap::new(),
        }
    }
}

pub struct NodeState<'a> {
    ctx: &'a NodeContext,
    persistent_state: PersistentState,
    volatile_state: VolatileState,
    leader_state: Option<LeaderState>,
}

impl<'a> NodeState<'a> {
    pub fn new(ctx: &'a NodeContext) -> Result<Self> {
        let persistent_state = PersistentState::new(ctx)?;

        Ok(NodeState {
            ctx,
            persistent_state,
            volatile_state: VolatileState::new(ctx),
            leader_state: None,
        })
    }

    pub fn get_current_term(&self) -> u64 {
        self.persistent_state.current_term
    }

    pub fn increment_term(&mut self) -> Result<u64> {
        self.persistent_state.current_term += 1;
        self.persistent_state.write_to_disk(self.ctx)?;
        Ok(self.persistent_state.current_term)
    }

    pub fn vote_for(&mut self, node: &str) -> Result<()> {
        self.persistent_state.voted_for = Some(node.to_string());
        self.persistent_state.write_to_disk(self.ctx);
        Ok(())
    }

    pub fn clear_vote(&mut self) -> Result<()> {
        self.persistent_state.voted_for = None;
        self.persistent_state.write_to_disk(self.ctx);
        Ok(())
    }

    pub fn to_follower(&mut self) {
        self.volatile_state.role = NodeRole::Follower;
    }

    pub fn to_candidate(&mut self) {
        self.volatile_state.role = NodeRole::Candidate;
    }

    pub fn to_leader(&mut self) {
        self.volatile_state.role = NodeRole::Leader;
    }

    pub fn get_last_logged_term_and_index(&self) -> (u64, u64) {
        (
            self.volatile_state.last_logged_term,
            self.volatile_state.last_logged_index,
        )
    }

    pub fn get_election_timeout(&self) -> u32 {
        self.volatile_state.current_election_timeout_ms
    }

    pub fn randomize_election_timeout(&mut self) -> u32 {
        self.volatile_state.current_election_timeout_ms = generate_election_timeout_ms(
            self.ctx.election_timeout_min,
            self.ctx.election_timeout_max,
        );
        self.volatile_state.current_election_timeout_ms
    }
}

fn generate_election_timeout_ms(min: u32, max: u32) -> u32 {
    rand::random_range(min..=max)
}
