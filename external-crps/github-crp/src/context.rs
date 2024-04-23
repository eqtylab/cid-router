use std::sync::Arc;

use anyhow::Result;
use octocrab::Octocrab;

use crate::{
    config::{Config, IndexingStrategy, RepoFilter},
    db::Db,
};

pub struct Context {
    pub start_time: i64,
    pub port: u16,
    pub indexing_strategy: IndexingStrategy,
    pub repos: Vec<RepoFilter>,
    pub db: Arc<Db>,
    pub octocrab: Arc<Octocrab>,
}

impl Context {
    pub fn init(config: Config) -> Result<Self> {
        let start_time = chrono::Utc::now().timestamp();

        let port = config.port;

        let indexing_strategy = config.indexing_strategy;

        let repos = config.repos;

        let db = Arc::new(Db::init(config.db_file)?);

        let octocrab = octocrab::instance();

        Ok(Self {
            start_time,
            port,
            indexing_strategy,
            repos,
            db,
            octocrab,
        })
    }
}
