use clap::Parser;

/// Error diagnosis assistant — pipe errors or paste them for LLM-powered help
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Error message to diagnose
    #[arg(default_value = "")]
    pub input: String,

    /// Include deeper context, common causes, and docs
    #[arg(short, long)]
    pub verbose: bool,

    /// Show multiple fix approaches with trade-offs
    #[arg(short, long)]
    pub alt: bool,

    /// Execute the suggested fix command after confirmation
    #[arg(short = 'x', long)]
    pub execute: bool,

    /// Configure provider and model
    #[arg(long)]
    pub config: bool,

    /// Upgrade to latest version from git
    #[arg(long)]
    pub upgrade: bool,
}
