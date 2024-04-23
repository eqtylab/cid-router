use std::path::PathBuf;

use clap::Parser;

/// azure-blob-storage-crp
#[derive(Debug, Clone, Parser)]
#[clap(version, about, long_about = None)]
#[clap(name = "azure-blob-storage-crp")]
pub struct Args {
    #[clap(subcommand)]
    pub cmd: Subcommand,
}

/// CLI Args top-level Subcommand
#[derive(Debug, Clone, Parser)]
pub enum Subcommand {
    Start(Start),
}

/// Start service
#[derive(Debug, Clone, Parser)]
pub struct Start {
    #[clap(flatten)]
    pub common_args: CommonArgs,
}

/// Common Args
#[derive(Debug, Clone, Parser)]
pub struct CommonArgs {
    /// Config file to use
    #[clap(short, long)]
    pub config: PathBuf,
}
