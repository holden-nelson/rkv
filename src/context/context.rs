use std::{net::SocketAddr, path::PathBuf};

use directories::ProjectDirs;

use crate::config::config::{self, Config};

#[derive(Debug)]
pub struct NodeContext {
    id: String,
    client_addr: SocketAddr,
    raft_addr: SocketAddr,
    role: NodeRole,

    persistence: PersistenceContext,
    peers: Vec<ClusterMember>,
}

#[derive(Debug)]
pub enum NodeRole {
    Leader,
    Candidate,
    Follower,
}

#[derive(Debug)]
pub struct PersistenceContext {
    base_dir: PathBuf,
}

#[derive(Debug)]
pub struct ClusterMember {
    id: String,
    client_addr: SocketAddr,
    raft_addr: SocketAddr,
}

#[derive(Debug)]
pub enum MemberValidationResult {
    Success(ClusterMember),
    InvalidRaftAddr(String),
    InvalidClientAddr(String),
}

#[derive(Debug)]
pub enum ClusterValidationResult {
    Success(NodeContext),
    EvenMembersWarning(NodeContext, usize),
    InvalidClusterMember(MemberValidationResult),
    InvalidId(String),
    DuplicateIds(String),
}

impl NodeContext {
    pub fn from_config(cfg: Config, node_id: &str) -> ClusterValidationResult {
        let binding = ProjectDirs::from("", "", "rkv").expect("failed to open project dir");
        let proj_path = binding.data_local_dir();

        from_config_with(cfg, node_id, proj_path.to_path_buf())
    }
}

fn from_config_with(cfg: Config, node_id: &str, project_path: PathBuf) -> ClusterValidationResult {
    let mut peers = vec![];

    let mut this_member: Option<ClusterMember> = None;
    for member in &cfg.members {
        match validate_config_member(member) {
            MemberValidationResult::Success(member) => {
                if member.id == node_id {
                    if this_member.is_some() {
                        return ClusterValidationResult::DuplicateIds(node_id.to_string());
                    }
                    this_member = Some(member);
                } else {
                    peers.push(member);
                }
            }
            other => return ClusterValidationResult::InvalidClusterMember(other),
        }
    }

    if this_member.is_none() {
        return ClusterValidationResult::InvalidId(node_id.to_string());
    }

    let data_dir = project_path.join("member-data").join(node_id);

    let this_member = this_member.unwrap();

    let context = NodeContext {
        id: this_member.id,
        client_addr: this_member.client_addr,
        raft_addr: this_member.raft_addr,
        role: NodeRole::Follower,
        persistence: PersistenceContext { base_dir: data_dir },
        peers: peers,
    };

    let num_peers = context.peers.len();
    if num_peers % 2 == 1 {
        return ClusterValidationResult::EvenMembersWarning(context, num_peers + 1);
    }

    return ClusterValidationResult::Success(context);
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

fn build_data_dir(node_id: &str) -> Option<PathBuf> {
    let proj_dir = ProjectDirs::from("", "", "rkv")?;
    Some(proj_dir.data_local_dir().join("member-data").join(node_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::config;
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
        };

        match from_config_with(cfg, "n3", stub_project_path()) {
            ClusterValidationResult::InvalidId(id) => assert_eq!(id, "n3"),
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
        };

        match from_config_with(cfg, "n1", stub_project_path()) {
            ClusterValidationResult::DuplicateIds(id) => assert_eq!(id, "n1"),
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
        };

        match from_config_with(cfg, "n1", stub_project_path()) {
            ClusterValidationResult::InvalidClusterMember(
                MemberValidationResult::InvalidClientAddr(s),
            ) => assert_eq!(s, "definitely-not-an-addr"),
            other => panic!("expected InvalidClusterMember(InvalidClientAddr), got {other:?}"),
        }
    }

    #[test]
    fn from_config_with_warns_on_even_member_count() {
        // Total members = 2 => peers.len() for a node = 1 (odd) => EvenMembersWarning(..., 2)
        let cfg = config::Config {
            members: vec![cfg_member("n1", 7001, 7000), cfg_member("n2", 8001, 8000)],
        };

        match from_config_with(cfg, "n1", stub_project_path()) {
            ClusterValidationResult::EvenMembersWarning(ctx, total) => {
                assert_eq!(total, 2);
                assert_eq!(ctx.id, "n1");
                assert!(matches!(ctx.role, NodeRole::Follower));
                assert_eq!(ctx.peers.len(), 1);
                assert_eq!(ctx.peers[0].id, "n2");
            }
            other => panic!("expected EvenMembersWarning, got {other:?}"),
        }
    }

    #[test]
    fn from_config_with_succeeds_on_odd_member_count() {
        // Total members = 3 => peers.len() for a node = 2 (even) => Success
        let cfg = config::Config {
            members: vec![
                cfg_member("n1", 7001, 7000),
                cfg_member("n2", 8001, 8000),
                cfg_member("n3", 9001, 9000),
            ],
        };

        match from_config_with(cfg, "n1", stub_project_path()) {
            ClusterValidationResult::Success(ctx) => {
                assert_eq!(ctx.id, "n1");
                assert!(matches!(ctx.role, NodeRole::Follower));
                assert_eq!(ctx.peers.len(), 2);
                assert!(ctx.peers.iter().any(|m| m.id == "n2"));
                assert!(ctx.peers.iter().any(|m| m.id == "n3"));

                // Ensure the persistence path is derived from the injected project_path + node id.
                assert!(
                    ctx.persistence
                        .base_dir
                        .ends_with(PathBuf::from("member-data").join("n1"))
                );
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }
}
