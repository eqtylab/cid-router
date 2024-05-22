use std::sync::Arc;

use anyhow::Result;
use tokio::time::{Duration, Instant};

use crate::{config::IndexingStrategy, context::Context};

pub async fn start(ctx: Arc<Context>) -> Result<()> {
    let ctx = ctx.clone();

    match blob_indexer_task(ctx).await {
        Err(e) => {
            panic!("blob_indexer_task error: {:?}", e);
        }
        Ok(()) => {
            panic!("blob_indexer_task returned, it should never return");
        }
    }
}

async fn blob_indexer_task(ctx: Arc<Context>) -> Result<()> {
    let Context { db, .. } = &*ctx;

    match ctx.indexing_strategy {
        IndexingStrategy::PollInterval(interval) => {
            let interval = Duration::from_secs(interval);

            loop {
                let next_update_time = Instant::now() + interval;

                if let Err(e) = db.update_blob_index(&ctx.blob_storage_config).await {
                    log::error!("Error updating blob index: {:?}", e);
                }
                if let Err(e) = db.update_blob_index_hashes(&ctx.blob_storage_config).await {
                    log::error!("Error updating blob index hashes: {:?}", e);
                }
                if let Err(e) = db.update_iroh_collections_index(&ctx.blob_storage_config) {
                    log::error!("Error updating iroh collections index: {:?}", e);
                }

                if Instant::now() < next_update_time {
                    tokio::time::sleep_until(next_update_time).await;
                }
            }
        }
    }
}
