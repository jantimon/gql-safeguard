mod args;
mod output;

use args::{Args, OutputFormat};
use clap::Parser;
use gql_safeguard_lib::analyzer::analyze_directory;
use output::format_violations;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.verbose {
        println!("Scanning path: {}", args.path.display());
        println!("Pattern: {}", args.pattern);
        println!("Output format: {:?}", args.output);
    }

    let violations = analyze_directory(&args.path, &args.pattern)?;

    let output = format_violations(&violations, &args.output, args.show_trees);
    println!("{}", output);

    // Exit with error code if violations found
    if !violations.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
