use std::sync::Arc;

use anyhow::Result;
use cid_router::{api, cli, config::Config, context::Context};
use clap::Parser;
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    match args.cmd {
        cli::Subcommand::Start(args) => start(args).await?,
    }

    Ok(())
}

async fn start(args: cli::Start) -> Result<()> {
    let config = Config::from_file(args.config)?;

    env_logger::init();

    info!("Starting: {config:#?}");

    let ctx = Context::init_from_config(config).await?;

    api::start(Arc::new(ctx)).await?;

    Ok(())
}
