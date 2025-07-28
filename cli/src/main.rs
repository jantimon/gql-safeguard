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
            let validation_result = validate_query_directives(&dependency_graph);
            if validation_result.is_valid() {
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
            } else {
                for error in &validation_result.errors {
                    println!("{}", error);
                }

                let elapsed = start_time.elapsed();
                println!();
                println!("💡 About @catch and @throwOnFieldError:");
                println!(
                    "The @throwOnFieldError directive requires protection by a @catch directive"
                );
                println!(
                    "in an ancestor field or a parent GraphQL fragment. Without proper @catch"
                );
                println!("protection, field errors will throw exceptions that bubble up and can");
                println!("break the entire Page");
                println!();
                println!("Fix by adding @catch to a parent field or fragment.");
                println!("Learn more: https://relay.dev/docs/next/guides/throw-on-field-error-directive/");
                println!();
                println!("❌ Validation failed: (took {:.2?})", elapsed);
                println!(
                    "Found {} queries and {} fragments",
                    registry.queries.len(),
                    registry.fragments.len()
                );
                std::process::exit(1);
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
