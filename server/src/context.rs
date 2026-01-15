use std::sync::Arc;

use anyhow::Result;
use cid_router_core::{crp::Crp, indexer::Indexer, repo::Repo};
use crp_azure::Container as AzureContainer;
use crp_iroh::IrohCrp;
use futures::future;

use crate::{
    auth::Auth,
    config::{Config, ProviderConfig},
};

pub struct Context {
    pub start_time: i64,
    pub port: u16,
    pub auth: Auth,
    pub core: cid_router_core::context::Context,
    pub providers: Vec<Arc<dyn Crp>>,
    pub indexer: Indexer,
}

impl Context {
    pub async fn init_from_repo(repo: Repo, config: Config) -> Result<Self> {
        let core = cid_router_core::context::Context::from_repo(repo).await?;
        Self::init_inner(core, config).await
    }

    /// Initialize context with an in-memory database (for testing).
    pub async fn init_in_memory(config: Config) -> Result<Self> {
        let core = cid_router_core::context::Context::mem().await?;
        Self::init_inner(core, config).await
    }

    async fn init_inner(core: cid_router_core::context::Context, config: Config) -> Result<Self> {
        let start_time = chrono::Utc::now().timestamp();
        let port = config.port;

        let auth = config.auth.clone();

        let providers = future::join_all(config.providers.into_iter().map(
            |provider_config| async move {
                match provider_config {
                    ProviderConfig::Iroh(iroh_config) => {
                        Ok(Arc::new(IrohCrp::new_from_config(iroh_config).await?) as Arc<dyn Crp>)
                    }
                    ProviderConfig::Azure(azure_config) => {
                        Ok(Arc::new(AzureContainer::new(azure_config)) as Arc<dyn Crp>)
                    }
                }
            },
        ))
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

        let indexer = Indexer::spawn(3600, core.clone(), providers.clone()).await;

        Ok(Self {
            start_time,
            port,
            auth,
            core,
            providers,
            indexer,
        })
    }
}
