use std::{fs, path::PathBuf};

use anyhow::Result;
use crp_iroh::IrohCrpConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum ProviderConfig {
    Iroh(IrohCrpConfig),
    // TODO: More CRP types
}

impl Config {
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let config = toml::from_str(&fs::read_to_string(path)?)?;

        Ok(config)
    }
}
