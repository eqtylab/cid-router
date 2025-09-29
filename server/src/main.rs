use std::{fs, path::PathBuf, sync::Arc};

use anyhow::Result;
use cid_router_core::repo::Repo;
use cid_router_server::{api, cli, config::Config, context::Context};
use clap::Parser;
use log::{info, warn};
use serde_json::Value;
use utoipa::openapi::{Info, OpenApi, Paths};

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    match args.cmd {
        cli::Subcommand::Start(args) => start(args).await?,
        cli::Subcommand::Openapi(args) => openapi(args).await?,
    }

    Ok(())
}

async fn start(args: cli::Start) -> Result<()> {
    let repo_path = args
        .repo_path
        .or(Some(cid_router_core::repo::Repo::default_location()))
        .unwrap();
    let repo = Repo::open_or_create(repo_path.clone()).await?;

    let server_config_path = repo_path.join("server.toml");
    let config = match server_config_path.exists() {
        true => Config::from_file(server_config_path)?,
        false => {
            warn!("config file does not exist. creating new config");
            tokio::fs::create_dir_all(&repo_path).await?;
            Config::default().write(server_config_path).await?
        }
    };

    env_logger::init();

    info!("Starting: {config:#?}");

    let ctx = Context::init_from_repo(repo, config).await?;

    api::start(Arc::new(ctx)).await?;

    Ok(())
}

async fn openapi(args: cli::Openapi) -> Result<()> {
    let dir = args.dir.unwrap_or(PathBuf::from("."));

    let mut openapi = OpenApi::new(Info::new("CID Router", "0.1.0"), Paths::new());

    let file_path = dir.join("cid-router.json");
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let api = api::openapi();
    openapi.merge(api);

    let content =
        serde_json::to_string_pretty(&serde_json::from_str::<Value>(&openapi.to_json()?)?)?;

    fs::write(file_path, content)?;

    Ok(())
}
