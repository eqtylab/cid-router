use std::str::FromStr;

use anyhow::Result;

use crate::config::Config;

pub fn init(config: &Config) -> Result<()> {
    let log_level_default =
        log::LevelFilter::from_str(config.log_level_default.as_deref().unwrap_or("error"))?;
    let log_level_app =
        log::LevelFilter::from_str(config.log_level_app.as_deref().unwrap_or("info"))?;

    env_logger::Builder::new()
        .filter_level(log_level_default)
        .filter_module("azure_blob_storage_crp", log_level_app)
        .init();

    Ok(())
}
