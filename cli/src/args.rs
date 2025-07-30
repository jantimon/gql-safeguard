use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "gql-safeguard")]
#[command(about = "Analyze GraphQL operations for missing @catch directives")]
#[command(version)]
pub struct Args {
    // Root directory for GraphQL extraction
    #[arg(default_value = ".")]
    pub path: PathBuf,

    // Which files contain GraphQL template literals
    #[arg(long, default_value = "**/*.{ts,tsx}")]
    pub pattern: String,

    // Skip build artifacts and dependencies
    #[arg(long)]
    pub ignore: Option<String>,

    // Enable project-relative path execution
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    // Primary operation mode
    #[command(subcommand)]
    pub command: Command,

    // Enable debug output for troubleshooting
    #[arg(long, short)]
    pub verbose: bool,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    // Check @throwOnFieldError protection patterns
    Validate {
        // Display fragment resolution for debugging
        #[arg(long)]
        show_trees: bool,
        // Output results in JSON format for programmatic use
        #[arg(long)]
        json: bool,
    },
    // Export extracted GraphQL for external tools
    Json,
}
