use std::{collections::HashSet, io, path::PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

use crate::core::{events::Event, storage::atomic_write};

pub struct NodeLifecycleManager {
    node_id: String,
    event_tx: Sender<Event>,
    current_role: Role,
    persistent_state: PersistentState,
    election_majority: u32,
    pub commit_index: u64,
}

impl NodeLifecycleManager {
    pub fn new() -> Self {}

    pub async fn record_vote(&mut self, vote: Vote) {
        if let Role::Candidate { election } = &mut self.current_role {
            match vote.granted {
                true => election.record_granted(vote.node_id),
                false => election.record_denied(vote.node_id),
            };

            if election.granted_count() >= self.election_majority {
                let victory_event = Event::ElectionVictory;
                self.event_tx.send(victory_event).await;
            }
        }
    }

    pub fn get_current_term(&self) -> u64 {
        self.persistent_state.get_current_term()
    }

    fn to_candidate(&mut self) {
        self.current_role = Role::Candidate {
            election: Election::new(self.node_id.clone()),
        };
        self.persistent_state.increment_term();
    }

    fn to_leader(&mut self) {
        self.current_role = Role::Leader;
    }

    fn to_follower(&mut self, term: u64) -> io::Result<()> {
        self.current_role = Role::Follower;
        self.persistent_state.update_term(term)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct PersistentState {
    #[serde(skip)]
    path: PathBuf,
    current_term: u64,
    voted_for: Option<String>,
}

impl PersistentState {
    fn new(dir: PathBuf) -> io::Result<Self> {
        let state = PersistentState {
            path: dir.join("state.json"),
            current_term: 0,
            voted_for: None,
        };

        state.write_to_disk()?;

        Ok(state)
    }

    fn write_to_disk(&self) -> io::Result<()> {
        let bytes = serde_json::to_vec(&self)?;
        atomic_write(&self.path, &bytes)
    }

    fn get_current_term(&self) -> u64 {
        self.current_term
    }

    fn increment_term(&mut self) -> io::Result<u64> {
        self.current_term += 1;
        self.write_to_disk()?;
        Ok(self.current_term)
    }

    fn update_term(&mut self, term: u64) -> io::Result<u64> {
        self.current_term = term;
        self.write_to_disk()?;
        Ok(term)
    }

    fn vote_for(&mut self, node_id: &str) -> io::Result<bool> {
        if self.has_voted() {
            return Ok(false);
        }

        self.voted_for = Some(node_id.to_string());
        self.write_to_disk()?;
        Ok(true)
    }

    fn has_voted(&self) -> bool {
        self.voted_for.is_some()
    }
}

struct Election {
    responded: HashSet<String>,
    granted: HashSet<String>,
}

impl Election {
    fn new(node_id: String) -> Self {
        let mut e = Election {
            responded: HashSet::new(),
            granted: HashSet::new(),
        };
        e.record_granted(node_id);
        e
    }

    fn record_granted(&mut self, from: String) -> bool {
        let first = self.responded.insert(from.clone());
        self.granted.insert(from);
        first
    }

    fn record_denied(&mut self, from: String) -> bool {
        self.responded.insert(from)
    }

    fn granted_count(&self) -> u32 {
        self.granted.len() as u32
    }
}

struct Vote {
    node_id: String,
    granted: bool,
}

enum Role {
    Follower,
    Candidate { election: Election },
    Leader,
}
