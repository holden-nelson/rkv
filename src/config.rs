use anyhow::Result;
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub election_timeout: ElectionTimeoutConfig,
    pub members: Vec<ClusterMember>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterMember {
    pub id: String,
    pub client_addr: String,
    pub raft_addr: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElectionTimeoutConfig {
    pub minimum_ms: u32,
    pub maximum_ms: u32,
}

pub fn load_config(path: impl AsRef<Path>) -> Result<Config> {
    let s = fs::read_to_string(path)?;
    let cfg: Config = toml::from_str(&s)?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn write_temp_file(contents: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        path.push(format!("rkv_load_config_test_{nanos}.toml"));
        fs::write(&path, contents).expect("failed to write temp config file");
        path
    }

    #[test]
    fn load_config_reads_valid_toml_into_config_struct() {
        let toml = r#"
election_timeout = { minimum_ms = 300, maximum_ms = 500 }

members = [
  { id = "n1", raft_addr = "127.0.0.1:7001", client_addr = "127.0.0.1:7000" },
  { id = "n2", raft_addr = "127.0.0.1:8001", client_addr = "127.0.0.1:8000" }
]
"#;

        let path = write_temp_file(toml);

        let cfg = load_config(&path).expect("expected load_config to succeed");

        // cleanup
        let _ = fs::remove_file(&path);

        assert_eq!(cfg.election_timeout.minimum_ms, 300);
        assert_eq!(cfg.election_timeout.maximum_ms, 500);

        assert_eq!(cfg.members.len(), 2);

        assert_eq!(cfg.members[0].id, "n1");
        assert_eq!(cfg.members[0].client_addr, "127.0.0.1:7000");
        assert_eq!(cfg.members[0].raft_addr, "127.0.0.1:7001");

        assert_eq!(cfg.members[1].id, "n2");
        assert_eq!(cfg.members[1].client_addr, "127.0.0.1:8000");
        assert_eq!(cfg.members[1].raft_addr, "127.0.0.1:8001");
    }

    #[test]
    fn load_config_returns_err_for_missing_file() {
        let mut path = std::env::temp_dir();
        path.push("rkv_this_file_should_not_exist_123456789.toml");

        let res = load_config(&path);
        assert!(res.is_err());
    }

    #[test]
    fn load_config_returns_err_for_invalid_toml() {
        let path = write_temp_file("this is not toml = = =");

        let res = load_config(&path);

        // cleanup
        let _ = fs::remove_file(&path);

        assert!(res.is_err());
    }
}
