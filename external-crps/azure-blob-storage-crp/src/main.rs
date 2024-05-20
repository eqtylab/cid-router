use std::sync::Arc;

use anyhow::Result;
use azure_blob_storage_crp::{api, cli, config::Config, context::Context, indexers::blob_indexer};
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
    let config = Config::from_file(args.common_args.config)?;

    azure_blob_storage_crp::log::init(&config)?;

    info!("Starting: {config:#?}");

    let ctx = Arc::new(Context::init(config)?);

    blob_indexer::start(ctx.clone()).await?;

    api::start(ctx).await?;

    Ok(())
}
