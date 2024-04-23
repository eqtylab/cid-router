use std::sync::Arc;

use anyhow::Result;
use tokio::time::{Duration, Instant};

use crate::{config::IndexingStrategy, context::Context};

pub async fn blob_indexer_task(ctx: Arc<Context>) -> Result<()> {
    let Context { db, .. } = &*ctx;

    match ctx.indexing_strategy {
        IndexingStrategy::PollInterval(interval) => {
            let interval = Duration::from_secs(interval);

            loop {
                let next_update_time = Instant::now() + interval;

                db.update_blob_index(&ctx.blob_storage_config).await?;
                db.update_blob_index_hashes(&ctx.blob_storage_config)
                    .await?;

                if Instant::now() < next_update_time {
                    tokio::time::sleep_until(next_update_time).await;
                }
            }
        }
    }
}
