use std::path::PathBuf;

use anyhow::{Context, Result};
use iroh::SecretKey;

use crate::db::Db;

/// A repo is a local disk store of state consumed & provided by the
/// cid-router-core. Configuration is treated as opaque data to be
/// fed to a higher-level consumer, whereas the database & secret key
/// are both created & consumed by the core itself.
pub struct Repo(PathBuf);

impl Repo {
    const DB_FILE: &str = "db.sqlite";
    const KEY_FILE: &str = "key";
    const CONFIG_FILE: &str = "config.toml";

    pub fn default_location() -> PathBuf {
        dirs_next::data_local_dir().unwrap().join("cid-router")
    }

    /// Opens or creates a repo at the given base directory.
    pub async fn open_or_create(base_dir: impl Into<PathBuf>) -> Result<Self> {
        let this = Self(base_dir.into());

        if !this.0.join(Self::KEY_FILE).exists() {
            tokio::fs::create_dir_all(&this.0).await?;
            this.create_key().await?;
        };

        Ok(this)
    }

    async fn create_key(&self) -> Result<SecretKey> {
        let key_file_path = self.0.join(Self::KEY_FILE);
        let key = SecretKey::generate(rand::rngs::OsRng);
        tokio::fs::write(key_file_path, key.to_bytes()).await?;
        Ok(key)
    }

    pub async fn db(&self) -> Result<Db> {
        let db_file_path = self.0.join(Self::DB_FILE);
        Db::open_or_create(db_file_path)
            .await
            .context("opening database")
    }

    /// reads the config file as a string
    pub async fn config_string(&self) -> Result<String> {
        let config_file_path = self.0.join(Self::CONFIG_FILE);
        tokio::fs::read_to_string(config_file_path)
            .await
            .context("reading config file")
    }

    pub async fn secret_key(&self) -> Result<SecretKey> {
        let key_file_path = self.0.join(Self::KEY_FILE);
        let key = tokio::fs::read(key_file_path).await?;
        let key = key.as_slice().try_into()?;
        Ok(SecretKey::from_bytes(key))
    }
}
