use std::{fs, path::PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub repos: Vec<RepoFilter>,
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
#[serde(rename_all = "snake_case")]
pub enum RepoFilter {
    Repo { owner: String, repo: String },
    OwnedBy(String),
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
}

impl RepoFilter {
    pub fn is_match(&self, owner: &str, repo: &str) -> bool {
        match self {
            Self::Repo {
                owner: f_owner,
                repo: f_repo,
            } => owner == f_owner && repo == f_repo,
            Self::OwnedBy(f_owner) => owner == f_owner,
            Self::And(fs) => fs.iter().all(|f| f.is_match(owner, repo)),
            Self::Or(fs) => fs.iter().any(|f| f.is_match(owner, repo)),
            Self::Not(filter) => !filter.is_match(owner, repo),
        }
    }

    pub fn get_repo_search_list(&self) -> Vec<(String, Option<String>)> {
        match self {
            Self::Repo { owner, repo } => vec![(owner.clone(), Some(repo.clone()))],
            Self::OwnedBy(owner) => vec![(owner.clone(), None)],
            Self::And(fs) | Self::Or(fs) => {
                fs.iter().flat_map(|f| f.get_repo_search_list()).collect()
            }
            Self::Not(_) => vec![],
        }
    }
}

impl Config {
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let config = toml::from_str(&fs::read_to_string(path)?)?;

        Ok(config)
    }
}
