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

    /// Output format
    #[arg(long, value_enum, default_value = "text")]
    pub output: OutputFormat,

    /// Show dependency trees in output
    #[arg(long)]
    pub show_trees: bool,

    /// Show processing details
    #[arg(long, short)]
    pub verbose: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Text,
    Json,
}
