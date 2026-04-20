use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::{net::SocketAddr, path::PathBuf};
use thiserror::Error;

use crate::config::{self, Config};

pub struct ConfigurationManager {
    pub id: String,
    pub client_addr: SocketAddr,
    pub raft_addr: SocketAddr,
    pub cluster_size: u32,
    pub heartbeat_interval: u32,
    pub base_dir: PathBuf,
    pub peers: Vec<ClusterMember>,

    election_timeout_min: u32,
    election_timeout_max: u32,
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

impl ConfigurationManager {
    pub fn from_config(cfg: Config, node_id: &str) -> Result<Self> {
        let proj_dirs = ProjectDirs::from("", "", "rkv")
            .context("could not determine project directories for app 'rkv'")?;

        let proj_path = proj_dirs.data_local_dir();

        from_config_with(cfg, node_id, proj_path.to_path_buf())
            .with_context(|| format!("failed to build NodeContext for node_id='{node_id}'"))
    }

    pub fn rand_election_timeout_ms(&self) -> u32 {
        rand::random_range(self.election_timeout_min..=self.election_timeout_max)
    }

    pub fn get_majority(&self) -> u32 {
        self.cluster_size / 2 + 1
    }
}

fn from_config_with(
    cfg: Config,
    node_id: &str,
    project_path: PathBuf,
) -> Result<ConfigurationManager, ClusterValidationError> {
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

    let context = ConfigurationManager {
        id: this_member.id,
        client_addr: this_member.client_addr,
        raft_addr: this_member.raft_addr,
        heartbeat_interval: cfg.heartbeat_interval,
        election_timeout_min: cfg.election_timeout.minimum_ms,
        election_timeout_max: cfg.election_timeout.maximum_ms,
        cluster_size: (peers.len() + 1) as u32,
        base_dir: data_dir,
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
