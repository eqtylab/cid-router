use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use cid_router_core::{crp::Crp, repo::Repo};
use crp_iroh::IrohCrp;
use futures::future;

use crate::config::{Config, ProviderConfig};

pub struct Context {
    pub start_time: i64,
    pub port: u16,
    pub core: cid_router_core::context::Context,
    pub providers: Vec<Arc<dyn Crp + Send + Sync>>,
}

impl Context {
    pub async fn init_from_config(path: PathBuf, config: Config) -> Result<Self> {
        let start_time = chrono::Utc::now().timestamp();
        let port = config.port;
        let repo = Repo::open_or_create(path).await?;
        let core = cid_router_core::context::Context::from_repo(repo).await?;

        let providers = future::join_all(config.providers.into_iter().map(
            |provider_config| async move {
                match provider_config {
                    ProviderConfig::Iroh(iroh_config) => Ok(Arc::new(
                        IrohCrp::new_from_config(serde_json::to_value(iroh_config)?).await?,
                    )
                        as Arc<dyn Crp + Send + Sync>),
                }
            },
        ))
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            start_time,
            port,
            core,
            providers,
        })
    }
}
