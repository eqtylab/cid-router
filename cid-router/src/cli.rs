use std::path::PathBuf;

use clap::{Parser, ValueHint};

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
    Openapi(Openapi),
}

/// Start service
#[derive(Debug, Clone, Parser)]
pub struct Start {
    /// Config file to use
    #[clap(short, long)]
    pub config: PathBuf,
}

/// Generate OpenAPI json documents
#[derive(Debug, Clone, Parser)]
pub struct Openapi {
    /// Directory to write json documents to
    #[clap(value_hint = ValueHint::AnyPath, value_parser)]
    pub dir: Option<PathBuf>,
}
