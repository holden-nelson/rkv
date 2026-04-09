use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;
use directories::ProjectDirs;
use thiserror::Error;

use crate::config::{self, Config};

#[derive(Debug)]
pub struct NodeContext {
    pub id: String,
    pub client_addr: SocketAddr,
    pub raft_addr: SocketAddr,
    pub election_timeout_min: u32,
    pub election_timeout_max: u32,
    pub cluster_size: u32,

    pub persistence: PersistenceContext,
    pub peers: Vec<ClusterMember>,
}

#[derive(Debug)]
pub struct PersistenceContext {
    pub base_dir: PathBuf,
}

#[derive(Debug)]
pub struct ClusterMember {
    pub id: String,
    pub client_addr: SocketAddr,
    pub raft_addr: SocketAddr,
}

#[derive(Debug)]
pub enum MemberValidationResult {
    Success(ClusterMember),
    InvalidRaftAddr(String),
    InvalidClientAddr(String),
}

#[derive(Debug, Error)]
pub enum ClusterValidationError {
    #[error("invalid cluster member: {0:?}")]
    InvalidClusterMember(MemberValidationResult),

    #[error("node id '{0}' was not found in cluster members")]
    InvalidId(String),

    #[error("duplicate node id '{0}' found in cluster members")]
    DuplicateIds(String),

    #[error("min election timeout ({0}) cannot be greater than max ({1})")]
    InvalidElectionTimeout(u32, u32),
}

impl NodeContext {
    pub fn from_config(cfg: Config, node_id: &str) -> anyhow::Result<NodeContext> {
        let proj_dirs = ProjectDirs::from("", "", "rkv")
            .context("could not determine project directories for app 'rkv'")?;

        let proj_path = proj_dirs.data_local_dir();

        from_config_with(cfg, node_id, proj_path.to_path_buf())
            .with_context(|| format!("failed to build NodeContext for node_id='{node_id}'"))
    }
}

fn from_config_with(
    cfg: Config,
    node_id: &str,
    project_path: PathBuf,
) -> Result<NodeContext, ClusterValidationError> {
    let mut peers = vec![];

    let mut this_member: Option<ClusterMember> = None;
    for member in &cfg.members {
        match validate_config_member(member) {
            MemberValidationResult::Success(member) => {
                if member.id == node_id {
                    if this_member.is_some() {
                        return Err(ClusterValidationError::DuplicateIds(node_id.to_string()));
                    }
                    this_member = Some(member);
                } else {
                    peers.push(member);
                }
            }
            other => return Err(ClusterValidationError::InvalidClusterMember(other)),
        }
    }

    if this_member.is_none() {
        return Err(ClusterValidationError::InvalidId(node_id.to_string()));
    }

    let data_dir = project_path.join("member-data").join(node_id);

    let this_member = this_member.unwrap();

    let context = NodeContext {
        id: this_member.id,
        client_addr: this_member.client_addr,
        raft_addr: this_member.raft_addr,
        election_timeout_min: cfg.election_timeout.minimum_ms,
        election_timeout_max: cfg.election_timeout.maximum_ms,
        cluster_size: (peers.len() + 1) as u32,
        persistence: PersistenceContext { base_dir: data_dir },
        peers: peers,
    };

    Ok(context)
}

fn validate_config_member(member: &config::ClusterMember) -> MemberValidationResult {
    let raft_addr: SocketAddr = match member.raft_addr.parse() {
        Ok(addr) => addr,
        Err(_) => return MemberValidationResult::InvalidRaftAddr(member.raft_addr.clone()),
    };

    let client_addr: SocketAddr = match member.client_addr.parse() {
        Ok(addr) => addr,
        Err(_) => {
            return MemberValidationResult::InvalidClientAddr(member.client_addr.clone());
        }
    };

    MemberValidationResult::Success(ClusterMember {
        id: member.id.to_string(),
        client_addr,
        raft_addr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{self, ElectionTimeoutConfig};
    use std::{env, net::SocketAddr, path::PathBuf};

    fn stub_project_path() -> PathBuf {
        env::temp_dir().join("rkv-test-stub-project-path")
    }

    fn cfg_member(id: &str, raft_port: u16, client_port: u16) -> config::ClusterMember {
        config::ClusterMember {
            id: id.to_string(),
            raft_addr: format!("127.0.0.1:{raft_port}"),
            client_addr: format!("127.0.0.1:{client_port}"),
        }
    }

    #[test]
    fn validate_config_member_success_parses_addrs() {
        let member = cfg_member("n1", 7001, 7000);

        match validate_config_member(&member) {
            MemberValidationResult::Success(m) => {
                assert_eq!(m.id, "n1");
                assert_eq!(m.raft_addr, "127.0.0.1:7001".parse::<SocketAddr>().unwrap());
                assert_eq!(
                    m.client_addr,
                    "127.0.0.1:7000".parse::<SocketAddr>().unwrap()
                );
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn validate_config_member_invalid_raft_addr() {
        let member = config::ClusterMember {
            id: "n1".to_string(),
            raft_addr: "not-a-socket-addr".to_string(),
            client_addr: "127.0.0.1:7000".to_string(),
        };

        match validate_config_member(&member) {
            MemberValidationResult::InvalidRaftAddr(s) => assert_eq!(s, "not-a-socket-addr"),
            other => panic!("expected InvalidRaftAddr, got {other:?}"),
        }
    }

    #[test]
    fn validate_config_member_invalid_client_addr() {
        let member = config::ClusterMember {
            id: "n1".to_string(),
            raft_addr: "127.0.0.1:7001".to_string(),
            client_addr: "nope".to_string(),
        };

        match validate_config_member(&member) {
            MemberValidationResult::InvalidClientAddr(s) => assert_eq!(s, "nope"),
            other => panic!("expected InvalidClientAddr, got {other:?}"),
        }
    }

    #[test]
    fn from_config_with_returns_invalid_id_when_missing() {
        let cfg = config::Config {
            members: vec![cfg_member("n1", 7001, 7000), cfg_member("n2", 8001, 8000)],
            election_timeout: ElectionTimeoutConfig {
                minimum_ms: 300,
                maximum_ms: 500,
            },
        };

        match from_config_with(cfg, "n3", stub_project_path()) {
            Err(ClusterValidationError::InvalidId(id)) => assert_eq!(id, "n3"),
            other => panic!("expected InvalidId, got {other:?}"),
        }
    }

    #[test]
    fn from_config_with_returns_duplicate_ids_when_multiple_matches() {
        let cfg = config::Config {
            members: vec![
                cfg_member("n1", 7001, 7000),
                cfg_member("n1", 7002, 7002),
                cfg_member("n2", 8001, 8000),
            ],
            election_timeout: ElectionTimeoutConfig {
                minimum_ms: 300,
                maximum_ms: 500,
            },
        };

        match from_config_with(cfg, "n1", stub_project_path()) {
            Err(ClusterValidationError::DuplicateIds(id)) => assert_eq!(id, "n1"),
            other => panic!("expected DuplicateIds, got {other:?}"),
        }
    }

    #[test]
    fn from_config_with_returns_invalid_cluster_member_for_bad_addr() {
        let cfg = config::Config {
            members: vec![
                config::ClusterMember {
                    id: "n1".to_string(),
                    raft_addr: "127.0.0.1:7001".to_string(),
                    client_addr: "definitely-not-an-addr".to_string(),
                },
                cfg_member("n2", 8001, 8000),
                cfg_member("n3", 9001, 9000),
            ],
            election_timeout: ElectionTimeoutConfig {
                minimum_ms: 300,
                maximum_ms: 500,
            },
        };

        match from_config_with(cfg, "n1", stub_project_path()) {
            Err(ClusterValidationError::InvalidClusterMember(
                MemberValidationResult::InvalidClientAddr(s),
            )) => assert_eq!(s, "definitely-not-an-addr"),
            other => panic!("expected InvalidClusterMember(InvalidClientAddr), got {other:?}"),
        }
    }
}
