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

    // Support project-relative execution
    if let Some(cwd) = &args.cwd {
        std::env::set_current_dir(cwd)?;
        if args.verbose {
            println!("Changed working directory to: {}", cwd.display());
        }
    }

    // Skip common build artifacts and dependencies
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

    // Memory-efficient processing for large codebases
    let patterns = vec![args.pattern.as_str()];
    let registry = process_glob(&args.path, &patterns, &ignore_patterns)?;

    match args.command {
        Command::Validate { show_trees } => {

            if args.verbose {
                let elapsed = start_time.elapsed();
                println!("Found {} files in {elapsed:.2?}", registry.file_count);
            }

            // Expand fragments for complete validation context
            let dependency_graph = registry_to_dependency_graph(&registry)?;

            if args.verbose {
                let elapsed = start_time.elapsed();
                println!("Found {} queries in {elapsed:.2?}", dependency_graph.len());
            }

            // Check @catch/@throwOnFieldError protection rules
            let validation_result = validate_query_directives(&dependency_graph);
            if validation_result.is_valid() {
                let elapsed = start_time.elapsed();
                println!("✅ All GraphQL queries pass validation! (took {elapsed:.2?})");
                println!(
                    "Found {} queries and {} fragments",
                    registry.queries.len(),
                    registry.fragments.len()
                );
            } else {
                for error in &validation_result.errors {
                    println!("{error}");
                }

                let elapsed = start_time.elapsed();
                println!();
                println!("❌ @throwOnFieldError must not be used outside of @catch");
                println!(
                    "Without @catch protection, field errors will throw exceptions that bubble up"
                );
                println!("and will break the entire page during client and server-side rendering.");
                println!();
                println!("The reason why @catch is enforced instead of Error Boundaries is that");
                println!("Error boundaries don't catch Errors during SSR");
                println!();
                println!("🫵  Fix this by adding @catch to a field or parent fragment.");
                println!("Learn more: https://relay.dev/docs/next/guides/throw-on-field-error-directive/");
                println!();
                println!("❌ Validation failed: (took {elapsed:.2?})");
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
            // Export extracted GraphQL for external analysis
            let json_output = serde_json::to_string_pretty(&registry)?;
            println!("{json_output}");
        }
    }

    Ok(())
}
