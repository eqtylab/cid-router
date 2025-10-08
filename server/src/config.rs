use std::{fs, path::PathBuf};

use anyhow::Result;
use cid_router_core::auth::Auth;
use crp_azure::ContainerConfig as AzureContainerConfig;
use crp_iroh::IrohCrpConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub auth: Auth,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum ProviderConfig {
    Iroh(IrohCrpConfig),
    Azure(AzureContainerConfig),
    // TODO: More CRP types
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            auth: Auth::default(),
            providers: vec![],
        }
    }
}

impl Config {
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let config = toml::from_str(&fs::read_to_string(path)?)?;

        Ok(config)
    }

    pub async fn write(self, path: PathBuf) -> Result<Self> {
        fs::write(path, toml::to_string_pretty(&self)?)?;

        Ok(self)
    }
}
