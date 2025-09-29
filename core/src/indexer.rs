use std::sync::Arc;

use log::{info, warn};

use crate::{crp::Crp, Context};

pub struct Indexer {
    _task: tokio::task::JoinHandle<()>,
}

impl Indexer {
    pub async fn spawn(interval: u64, cx: Context, providers: Vec<Arc<dyn Crp>>) -> Self {
        let task = tokio::spawn(async move {
            info!("Starting indexer");
            loop {
                for provider in &providers {
                    if let Err(err) = provider.reindex(&cx).await {
                        warn!(
                            "Error reindexing provider {}:{}: {}",
                            provider.provider_type().to_string(),
                            provider.provider_id(),
                            err
                        );
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
            }
        });
        Self { _task: task }
    }
}
