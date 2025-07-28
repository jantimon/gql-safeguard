mod args;

use args::{Args, Command};
use clap::Parser;
use gql_safeguard_lib::registry::process_glob;
use gql_safeguard_lib::registry_to_graph::registry_to_dependency_graph;
use gql_safeguard_lib::validate::validate_query_directives;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let start_time = Instant::now();
    let args = Args::parse();

    // Change working directory if specified
    if let Some(cwd) = &args.cwd {
        std::env::set_current_dir(cwd)?;
        if args.verbose {
            println!("Changed working directory to: {}", cwd.display());
        }
    }

    // Set default ignore pattern if none provided
    let ignore_patterns = match args.ignore.as_deref() {
        Some(pattern) => vec![pattern],
        None => vec![
            "**/node_modules",
            "**/.git",
            "**/.yarn",
            "**/.swc",
            "**/*.xcassets",
        ],
    };

    if args.verbose {
        println!("Scanning path: {}", args.path.display());
        println!("Pattern: {}", args.pattern);
        println!("Ignore pattern: {}", ignore_patterns.join(", "));
    }

    // Process files using the streaming approach
    let patterns = vec![args.pattern.as_str()];
    let registry = process_glob(&args.path, &patterns, &ignore_patterns)?;

    match args.command {
        Command::Validate { show_trees } => {
            // Build dependency graph
            let dependency_graph = registry_to_dependency_graph(&registry)?;

            if args.verbose {
                println!("Found {} queries", dependency_graph.len());
            }

            // Validate the queries
            match validate_query_directives(&dependency_graph) {
                Ok(()) => {
                    let elapsed = start_time.elapsed();
                    println!(
                        "✅ All GraphQL queries pass validation! (took {:.2?})",
                        elapsed
                    );
                    println!(
                        "Found {} queries and {} fragments",
                        registry.queries.len(),
                        registry.fragments.len()
                    );
                }
                Err(e) => {
                    let elapsed = start_time.elapsed();
                    println!("❌ Validation failed: {} (took {:.2?})", e, elapsed);
                    println!(
                        "Found {} queries and {} fragments",
                        registry.queries.len(),
                        registry.fragments.len()
                    );
                    std::process::exit(1);
                }
            }

            if show_trees {
                println!("\n--- Dependency Trees ---");
                for query in &dependency_graph {
                    println!("Query: {} ({})", query.name, query.file_path.display());
                    // Could add tree formatting here
                }
            }
        }
        Command::Json => {
            // Serialize registry to JSON
            let json_output = serde_json::to_string_pretty(&registry)?;
            println!("{}", json_output);
        }
    }

    Ok(())
}
