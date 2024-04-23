use std::sync::Arc;

use anyhow::Result;
use azure_blob_storage_crp::{api, cli, config::Config, context::Context};
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

    tokio::spawn({
        let ctx = ctx.clone();

        async move {
            ctx.db.update_blob_index(&ctx.blob_storage_config).await?;
            ctx.db
                .update_blob_index_hashes(&ctx.blob_storage_config)
                .await?;
            anyhow::Ok(())
        }
    });

    api::start(ctx).await?;

    Ok(())
}
