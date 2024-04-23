use std::{fs, path::PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub blob_storage: BlobStorageConfig,
    pub indexing_strategy: IndexingStrategy,
    pub db_file: PathBuf,
    pub log_level_default: Option<String>,
    pub log_level_app: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexingStrategy {
    /// Update the index every `x` seconds
    PollInterval(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobStorageConfig {
    pub containers: Vec<ContainerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub account: String,
    pub container: String,
    pub filter: ContainerBlobFilter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerBlobFilter {
    All,
    Directory(String),
    FileExt(String),
    NameContains(String),
    Size { min: Option<u64>, max: Option<u64> },
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
}

impl ContainerBlobFilter {
    pub fn blob_is_match(&self, name: &str, size: u64) -> bool {
        match self {
            Self::All => true,
            Self::Directory(prefix) => name.starts_with(prefix),
            Self::FileExt(ext) => name.ends_with(&format!(".{ext}")),
            Self::NameContains(sub) => name.contains(sub),
            Self::Size { min, max } => match (min, max) {
                (Some(min), Some(max)) => size >= *min && size <= *max,
                (Some(min), None) => size >= *min,
                (None, Some(max)) => size <= *max,
                (None, None) => true,
            },
            Self::And(fs) => fs.iter().all(|f| f.blob_is_match(name, size)),
            Self::Or(fs) => fs.iter().any(|f| f.blob_is_match(name, size)),
            Self::Not(f) => !f.blob_is_match(name, size),
        }
    }
}

impl Config {
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let config = toml::from_str(&fs::read_to_string(path)?)?;

        Ok(config)
    }
}
