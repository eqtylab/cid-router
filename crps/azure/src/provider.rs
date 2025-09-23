use anyhow::Result;
use async_trait::async_trait;
use cid_router_core::{
    Context,
    cid_filter::{CidFilter, CodeFilter},
    crp::{Crp, CrpCapabilities, RoutesIndexer},
};
use iroh_blobs::{
    BlobFormat, Hash,
    get::request::{GetBlobItem, get_verified_size},
    ticket::BlobTicket,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{config::ContainerConfig, container::Container};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureCrpConfig {
    pub containers: Vec<ContainerConfig>,
}

#[derive(Debug)]
pub struct AzureService {
    containers: Vec<Container>,
}

impl AzureService {
    pub async fn new_from_config(config: Value) -> Result<Self> {
        let AzureCrpConfig { containers } = serde_json::from_value(config)?;

        let containers = containers.into_iter().map(Container::new).collect();

        Ok(Self { containers })
    }
}

#[async_trait]
impl Crp for AzureService {
    fn capabilities<'a>(&'a self) -> CrpCapabilities<'a> {
        CrpCapabilities {
            routes_indexer: Some(self),
            bytes_resolver: None, // TODO(b5)
            size_resolver: None,  // TODO(b5)
        }
    }

    fn cid_filter(&self) -> CidFilter {
        CidFilter::MultihashCodeFilter(CodeFilter::Eq(0x1e)) // blake3
    }
}

#[async_trait]
impl RoutesIndexer for AzureService {
    async fn reindex(&self, _cx: &Context) -> Result<()> {
        todo!();
    }
}
