use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use github_crp::{api, cli, config::Config, context::Context, indexers::commit_indexer};
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

    github_crp::log::init(&config)?;

    info!("Starting: {config:#?}");

    let ctx = Arc::new(Context::init(config)?);

    tokio::spawn(commit_indexer::update_commit_index(ctx.clone()));

    api::start(ctx).await?;

    Ok(())
}
