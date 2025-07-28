use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "graphql-directive-analyzer")]
#[command(about = "Analyze GraphQL operations for missing @catch directives")]
#[command(version)]
pub struct Args {
    /// Path to scan for TypeScript/TSX files
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// File glob pattern to match
    #[arg(long, default_value = "**/*.{ts,tsx}")]
    pub pattern: String,

    /// File glob pattern to ignore (defaults to "**/node_modules/**")
    #[arg(long)]
    pub ignore: Option<String>,

    /// Change working directory before executing
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    /// Command to execute
    #[command(subcommand)]
    pub command: Command,

    /// Show processing details
    #[arg(long, short)]
    pub verbose: bool,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Validate GraphQL operations for missing @catch directives
    Validate {
        /// Show dependency trees in output
        #[arg(long)]
        show_trees: bool,
    },
    /// Output registry information in JSON format
    Json,
}
