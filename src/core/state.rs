use std::{collections::HashMap, io, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{
    context::NodeContext,
    core::{log::LogStore, replication::FollowerReplicationState, storage::atomic_write},
};

#[derive(Serialize, Deserialize)]
struct PersistentState {
    #[serde(skip)]
    base_dir: PathBuf,
    current_term: u64,
    voted_for: Option<String>,
}

impl PersistentState {
    fn new(ctx: &NodeContext) -> Result<Self> {
        let state = PersistentState {
            base_dir: ctx.persistence.base_dir.clone(),
            current_term: 0,
            voted_for: None,
        };

        state.write_to_disk()?;

        Ok(state)
    }

    fn write_to_disk(&self) -> io::Result<()> {
        let bytes = serde_json::to_vec(&self)?;
        let path = self.base_dir.join("state.json");
        atomic_write(&path, &bytes)
    }
}

struct VolatileState {
    role: RoleState,
    commit_index: u64,
    last_applied: u64,
    min_election_timeout_ms: u32,
    max_election_timout_ms: u32,
    current_election_timeout_ms: u32,
    last_logged_index: u64,
    last_logged_term: u64,
}

impl VolatileState {
    fn new(ctx: &NodeContext) -> Self {
        VolatileState {
            role: RoleState::Follower,
            min_election_timeout_ms: ctx.election_timeout_min,
            max_election_timout_ms: ctx.election_timeout_max,
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

    fn randomize_election_timeout(&mut self) -> u32 {
        self.current_election_timeout_ms =
            generate_election_timeout_ms(self.min_election_timeout_ms, self.max_election_timout_ms);
        self.current_election_timeout_ms
    }
}

enum RoleState {
    Follower,
    Candidate {
        votes_received: u32,
    },
    Leader {
        replication_state: HashMap<String, FollowerReplicationState>,
        pending_client_requests_by_index: HashMap<u64, oneshot::Sender<Result<()>>>,
    },
}

pub struct NodeState {
    persistent_state: PersistentState,
    volatile_state: VolatileState,
    pub log_store: LogStore,
}

impl NodeState {
    pub fn new(ctx: &NodeContext, log_store: LogStore) -> Result<Self> {
        let persistent_state = PersistentState::new(ctx)?;

        Ok(NodeState {
            persistent_state,
            volatile_state: VolatileState::new(ctx),
            log_store,
        })
    }

    pub fn get_current_term(&self) -> u64 {
        self.persistent_state.current_term
    }

    pub fn increment_term(&mut self) -> Result<u64> {
        self.persistent_state.current_term += 1;
        self.persistent_state.write_to_disk()?;
        Ok(self.persistent_state.current_term)
    }

    pub fn vote_for(&mut self, node: &str) -> Result<()> {
        self.persistent_state.voted_for = Some(node.to_string());
        self.persistent_state.write_to_disk()?;
        Ok(())
    }

    pub fn clear_vote(&mut self) -> Result<()> {
        self.persistent_state.voted_for = None;
        self.persistent_state.write_to_disk()?;
        Ok(())
    }

    pub fn has_voted(&self) -> bool {
        self.persistent_state.voted_for.is_some()
    }

    pub fn get_vote_count(&self) -> Option<u32> {
        match &self.volatile_state.role {
            RoleState::Candidate { votes_received } => Some(*votes_received),
            _ => None,
        }
    }

    pub fn record_vote(&mut self) -> Option<u32> {
        match &mut self.volatile_state.role {
            RoleState::Candidate { votes_received } => {
                *votes_received += 1;
                Some(*votes_received)
            }
            _ => None,
        }
    }

    pub fn to_follower(&mut self) -> Result<()> {
        self.volatile_state.role = RoleState::Follower;
        self.clear_vote()?;

        Ok(())
    }

    pub fn to_candidate(&mut self) {
        self.volatile_state.role = RoleState::Candidate { votes_received: 0 };
    }

    pub fn to_leader(&mut self, ctx: &NodeContext) -> Result<()> {
        let last_index = self.log_store.last_index();
        let replication_state = ctx
            .peers
            .iter()
            .map(|peer| {
                (
                    peer.id.clone(),
                    FollowerReplicationState {
                        next_index: last_index + 1,
                        match_index: 0,
                        inflight: false,
                        needs_replicated_to: true,
                    },
                )
            })
            .collect();

        self.volatile_state.role = RoleState::Leader {
            replication_state,
            pending_client_requests_by_index: HashMap::new(),
        };
        self.clear_vote()?;

        Ok(())
    }

    pub fn get_last_logged_term_and_index(&mut self) -> Result<(u64, u64)> {
        Ok((self.log_store.last_term()?, self.log_store.last_index()))
    }

    pub fn get_commit_index(&self) -> u64 {
        self.volatile_state.commit_index
    }

    pub fn set_commit_index(&mut self, index: u64) {
        self.volatile_state.commit_index = index;
    }

    pub fn get_election_timeout(&self) -> u32 {
        self.volatile_state.current_election_timeout_ms
    }

    pub fn randomize_election_timeout(&mut self) -> u32 {
        self.volatile_state.randomize_election_timeout()
    }

    pub fn get_replication_state(&self) -> Option<&HashMap<String, FollowerReplicationState>> {
        match &self.volatile_state.role {
            RoleState::Leader {
                replication_state, ..
            } => Some(replication_state),
            _ => None,
        }
    }

    pub fn get_follower_replication_state_mut(
        &mut self,
        node_id: &str,
    ) -> Option<&mut FollowerReplicationState> {
        match &mut self.volatile_state.role {
            RoleState::Leader {
                replication_state, ..
            } => replication_state.get_mut(node_id),
            _ => None,
        }
    }
}

fn generate_election_timeout_ms(min: u32, max: u32) -> u32 {
    rand::random_range(min..=max)
}
