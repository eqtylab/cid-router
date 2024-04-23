use std::path::PathBuf;

use clap::Parser;

/// cid-router
#[derive(Debug, Clone, Parser)]
#[clap(version, about, long_about = None)]
#[clap(name = "cid-router")]
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
    /// Config file to use
    #[clap(short, long)]
    pub config: PathBuf,
}
