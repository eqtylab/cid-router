use std::{collections::HashMap, sync::Arc};

use anyhow::Result;

use crate::{
    config::{Config, ProviderConfig},
    crp::{external::ExternalCrp, ipfs::IpfsCrp, iroh::IrohCrp, Crp},
};

pub struct Context {
    pub start_time: i64,
    pub port: u16,
    pub providers: HashMap<String, Arc<dyn Crp + Send + Sync>>,
}

impl Context {
    pub async fn init_from_config(config: Config) -> Result<Self> {
        let start_time = chrono::Utc::now().timestamp();

        let port = config.port;

        let providers = {
            let ps = futures::future::join_all(config.providers.into_iter().map(
                |provider| async move {
                    let mut provider = match provider.clone() {
                        ProviderConfig::External(external_crp_config) => Box::new(
                            ExternalCrp::new_from_config(external_crp_config, provider)
                                .expect("failed to create an external crp from config"),
                        )
                            as Box<dyn Crp + Send + Sync>,
                        ProviderConfig::Ipfs(ipfs_crp_config) => Box::new(
                            IpfsCrp::new_from_config(ipfs_crp_config, provider)
                                .expect("failed to create an ipfs crp from config"),
                        )
                            as Box<dyn Crp + Send + Sync>,
                        ProviderConfig::Iroh(iroh_crp_config) => Box::new(
                            IrohCrp::new_from_config(iroh_crp_config, provider)
                                .await
                                .expect("failed to create an iroh crp from config"),
                        )
                            as Box<dyn Crp + Send + Sync>,
                    };
                    provider
                        .init()
                        .await
                        .expect("could not initialize provider");
                    provider
                },
            ))
            .await;

            ps.into_iter()
                .map(|provider| (provider.provider_id(), Arc::from(provider)))
                .collect::<HashMap<String, Arc<dyn Crp + Send + Sync>>>()
        };

        Ok(Self {
            start_time,
            port,
            providers,
        })
    }
}
