use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use cid_filter::{CidFilter, CodeFilter};
use reqwest::StatusCode;
use routes::{IntoRoute, IpfsRouteMethod, Route, UrlRouteMethod};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{config::ProviderConfig, crp::Crp};

#[derive(Debug)]
pub struct IpfsCrp {
    gateway_url: String,
    client: reqwest::Client,
    config: ProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpfsCrpConfig {
    pub gateway_url: String,
}

impl IpfsCrp {
    pub fn new_from_config(ipfs_crp_config: IpfsCrpConfig, config: ProviderConfig) -> Result<Self> {
        let IpfsCrpConfig { gateway_url } = ipfs_crp_config;
        let client = reqwest::Client::new();

        Ok(Self {
            gateway_url,
            client,
            config,
        })
    }
}

#[async_trait]
impl Crp for IpfsCrp {
    async fn init(&mut self) -> Result<()> {
        Ok(())
    }

    fn cid_filter(&self) -> CidFilter {
        CidFilter::CodecFilter(
            CodeFilter::Eq(0x70) // dag-pb
            | CodeFilter::Eq(0x71), // dag-cbor
        )
    }

    async fn get_routes_for_cid(&self, cid: &Cid) -> Result<Vec<Route>> {
        let Self { gateway_url, .. } = self;
        let cid = cid.to_string();

        let url = format!("{gateway_url}/ipfs/{cid}");

        let response = self.client.head(&url).send().await?;

        let crp_id = Some(self.provider_id());

        if response.status() == StatusCode::OK {
            Ok(vec![
                IpfsRouteMethod { cid }.into_route(crp_id.clone())?,
                UrlRouteMethod { url }.into_route(crp_id)?,
            ])
        } else {
            Ok(vec![])
        }
    }

    fn provider_config(&self) -> Value {
        serde_json::to_value(&self.config).expect("unexpectedly failed to serialize a config type")
    }
}
