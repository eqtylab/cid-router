use std::sync::Arc;

use anyhow::Result;

use crate::{
    config::{BlobStorageConfig, Config, IndexingStrategy},
    db::Db,
};

pub struct Context {
    pub start_time: i64,
    pub port: u16,
    pub indexing_strategy: IndexingStrategy,
    pub blob_storage_config: BlobStorageConfig,
    pub db: Arc<Db>,
}

impl Context {
    pub fn init(config: Config) -> Result<Self> {
        let start_time = chrono::Utc::now().timestamp();

        let port = config.port;

        let indexing_strategy = config.indexing_strategy;

        let blob_storage_config = config.blob_storage;

        let db = Arc::new(Db::init(config.db_file)?);

        Ok(Self {
            start_time,
            port,
            indexing_strategy,
            blob_storage_config,
            db,
        })
    }
}
