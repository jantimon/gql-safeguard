mod args;

use args::Args;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.verbose {
        println!("Scanning path: {}", args.path.display());
        println!("Pattern: {}", args.pattern);
        println!("Output format: {:?}", args.output);
    }

    Ok(())
}
