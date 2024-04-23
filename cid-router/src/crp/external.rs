use anyhow::{bail, Result};
use async_trait::async_trait;
use cid::Cid;
use cid_filter::CidFilter;
use reqwest::StatusCode;
use routes::Route;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{config::ProviderConfig, crp::Crp};

#[derive(Debug)]
pub struct ExternalCrp {
    base_url: String,
    client: reqwest::Client,
    filter: CidFilter,
    config: ProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCrpConfig {
    pub url: String,
}

impl ExternalCrp {
    pub fn new_from_config(
        external_crp_config: ExternalCrpConfig,
        config: ProviderConfig,
    ) -> Result<Self> {
        let ExternalCrpConfig { url: base_url } = external_crp_config;
        let client = reqwest::Client::new();
        let filter = CidFilter::None;

        Ok(Self {
            base_url,
            client,
            filter,
            config,
        })
    }
}

#[async_trait]
impl Crp for ExternalCrp {
    async fn init(&mut self) -> Result<()> {
        self.populate_filter().await?;

        Ok(())
    }

    fn cid_filter(&self) -> CidFilter {
        self.filter.clone()
    }

    async fn get_routes_for_cid(&self, cid: &Cid) -> Result<Vec<Route>> {
        let Self {
            base_url, client, ..
        } = self;

        let url = format!("{base_url}/routes/{cid}");

        let response = client.get(&url).send().await?;

        let routes = if response.status() == StatusCode::OK {
            let mut json = response.json::<Value>().await?;
            let routes = json["routes"].take();
            serde_json::from_value(routes)?
        } else {
            bail!("failed to fetch routes for CID: {}", response.text().await?);
        };

        Ok(routes)
    }

    fn provider_config(&self) -> Value {
        serde_json::to_value(&self.config).expect("unexpectedly failed to serialize a config type")
    }
}

impl ExternalCrp {
    async fn populate_filter(&mut self) -> Result<()> {
        let response = self
            .client
            .get(&format!("{}/filter", self.base_url))
            .send()
            .await?;

        let filter = if response.status() == StatusCode::OK {
            let mut json = response.json::<Value>().await?;
            let filter = json["filter"].take();
            serde_json::from_value(filter)?
        } else {
            bail!("failed to fetch filter");
        };

        self.filter = filter;

        Ok(())
    }
}
