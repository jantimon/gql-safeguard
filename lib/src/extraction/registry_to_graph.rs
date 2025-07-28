//! Registry to Dependency Graph Conversion
//!
//! This module transforms a GraphQL registry into a hierarchical dependency graph structure
//! where queries contain their fragment dependencies as nested trees. This enables easier
//! analysis of directive protection inheritance and dependency relationships.

use anyhow::{Context, Result};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::extraction::graphql_parser::{Directive, Field};
use crate::extraction::registry::GraphQLRegistry;

/// A query with its complete fragment dependency tree resolved
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryWithFragments {
    pub name: String,
    pub file_path: PathBuf,
    pub directives: Vec<Directive>,
    pub fields: Vec<Field>,
    pub fragment_tree: Vec<FragmentNode>,
}

/// A fragment node in the dependency tree with its children recursively resolved
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentNode {
    pub name: String,
    pub file_path: PathBuf,
    pub directives: Vec<Directive>,
    pub fields: Vec<Field>,
    pub children: Vec<FragmentNode>,
}

/// Converts a GraphQL registry into a dependency graph with resolved fragment trees
///
/// This function takes all queries from the registry and builds complete dependency trees
/// by recursively resolving fragment spreads. Each query becomes a `QueryWithFragments`
/// containing its nested fragment dependencies as a tree structure.
///
/// # Arguments
/// * `registry` - The GraphQL registry containing queries and fragments
///
/// # Returns
/// * `Vec<QueryWithFragments>` - All queries with their resolved fragment dependency trees
///
/// # Errors
/// * Returns error if circular dependencies are detected
/// * Returns error if a fragment spread references a non-existent fragment
pub fn registry_to_dependency_graph(registry: &GraphQLRegistry) -> Result<Vec<QueryWithFragments>> {
    let mut result = Vec::new();

    // Process each query in the registry
    for query_entry in registry.queries.iter() {
        let query_name = query_entry.key();
        let query = query_entry.value();

        // Build the fragment tree for this query
        let fragment_tree =
            resolve_fragment_dependencies(&query.fragments, registry, &mut FxHashSet::default())
                .with_context(|| {
                    format!("Failed to resolve dependencies for query '{}'", query_name)
                })?;

        result.push(QueryWithFragments {
            name: query.name.clone(),
            file_path: query.file_path.clone(),
            directives: query.directives.clone(),
            fields: query.fields.clone(),
            fragment_tree,
        });
    }

    // Sort by query name for consistent output
    result.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(result)
}

/// Recursively resolves fragment dependencies into a tree structure
///
/// This function takes a list of fragment spreads and builds a complete dependency tree
/// by recursively resolving each fragment's own fragment spreads. It maintains a
/// visiting set to detect circular dependencies.
///
/// # Arguments
/// * `fragment_spreads` - The fragment spreads to resolve
/// * `registry` - The GraphQL registry containing fragment definitions
/// * `visiting` - Set of fragment names currently being visited (for cycle detection)
///
/// # Returns
/// * `Vec<FragmentNode>` - The resolved fragment dependency tree
///
/// # Errors
/// * Returns error if circular dependency is detected
/// * Returns error if a fragment spread references a non-existent fragment
fn resolve_fragment_dependencies(
    fragment_spreads: &[crate::extraction::graphql_parser::FragmentSpread],
    registry: &GraphQLRegistry,
    visiting: &mut FxHashSet<String>,
) -> Result<Vec<FragmentNode>> {
    let mut fragment_nodes = Vec::new();

    for spread in fragment_spreads {
        // Check if this fragment exists in the registry
        let fragment_entry = registry
            .fragments
            .get(&spread.name)
            .with_context(|| format!("Fragment '{}' not found in registry", spread.name))?;

        let fragment = fragment_entry.value();

        // Check for circular dependency
        if visiting.contains(&spread.name) {
            return Err(anyhow::anyhow!(
                "Circular dependency detected involving fragment '{}'",
                spread.name
            ));
        }

        // Add to visiting set
        visiting.insert(spread.name.clone());

        // Recursively resolve this fragment's dependencies
        let children = resolve_fragment_dependencies(&fragment.fragments, registry, visiting)
            .with_context(|| {
                format!(
                    "Failed to resolve dependencies for fragment '{}'",
                    spread.name
                )
            })?;

        // Remove from visiting set (backtrack)
        visiting.remove(&spread.name);

        // Create the fragment node
        fragment_nodes.push(FragmentNode {
            name: fragment.name.clone(),
            file_path: fragment.file_path.clone(),
            directives: fragment.directives.clone(),
            fields: fragment.fields.clone(),
            children,
        });
    }

    // Sort by fragment name for consistent output
    fragment_nodes.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(fragment_nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extraction::registry::process_files;
    use crate::tree_formatter::TreeFormatter;
    use std::fs;
    use std::path::PathBuf;

    /// Formats a dependency graph using TreeFormatter for snapshot testing
    fn format_dependency_graph_with_tree_formatter(queries: &[QueryWithFragments]) -> String {
        let mut formatter = TreeFormatter::new();

        // Git root for relative paths in snapshots
        let git_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();

        formatter.add_line(0, "Dependency Graph");

        if queries.is_empty() {
            formatter.add_line(1, "No queries found");
            return formatter.to_string();
        }

        for query in queries {
            let relative_path = query
                .file_path
                .strip_prefix(&git_root)
                .unwrap_or(&query.file_path);
            formatter.add_line(1, &format!("{} ({})", query.name, relative_path.display()));

            // Query directives
            if !query.directives.is_empty() {
                formatter.add_line(2, "Directives:");
                for directive in &query.directives {
                    let emoji = match directive.directive_type {
                        crate::extraction::graphql_parser::DirectiveType::Catch => "🧤",
                        crate::extraction::graphql_parser::DirectiveType::ThrowOnFieldError => "⚠️",
                    };
                    formatter.add_line(3, &format!("{:?} {}", directive.directive_type, emoji));
                }
            }

            // Query fields
            if !query.fields.is_empty() {
                formatter.add_line(2, "Fields:");
                for field in &query.fields {
                    let mut field_text = field.name.clone();
                    if !field.directives.is_empty() {
                        let directive_strs: Vec<String> = field.directives.iter().map(|d| {
                            let emoji = match d.directive_type {
                                crate::extraction::graphql_parser::DirectiveType::Catch => "🧤",
                                crate::extraction::graphql_parser::DirectiveType::ThrowOnFieldError => "⚠️",
                            };
                            format!("{:?} {}", d.directive_type, emoji)
                        }).collect();
                        field_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                    }
                    formatter.add_line(3, &field_text);
                }
            }

            // Fragment tree
            if !query.fragment_tree.is_empty() {
                formatter.add_line(2, "Fragment Tree:");
                for fragment in &query.fragment_tree {
                    format_fragment_node(&mut formatter, fragment, 3, &git_root);
                }
            }
        }

        formatter.to_string()
    }

    /// Recursively formats a fragment node and its children
    fn format_fragment_node(
        formatter: &mut TreeFormatter,
        fragment: &FragmentNode,
        depth: usize,
        git_root: &PathBuf,
    ) {
        let relative_path = fragment
            .file_path
            .strip_prefix(git_root)
            .unwrap_or(&fragment.file_path);

        // Fragment name and location
        let mut fragment_text = format!("{} ({})", fragment.name, relative_path.display());

        // Add fragment-level directives to the name line if present
        if !fragment.directives.is_empty() {
            let directive_strs: Vec<String> = fragment
                .directives
                .iter()
                .map(|d| {
                    let emoji = match d.directive_type {
                        crate::extraction::graphql_parser::DirectiveType::Catch => "🧤",
                        crate::extraction::graphql_parser::DirectiveType::ThrowOnFieldError => "⚠️",
                    };
                    format!("{:?} {}", d.directive_type, emoji)
                })
                .collect();
            fragment_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
        }

        formatter.add_line(depth, &fragment_text);

        // Fragment fields
        if !fragment.fields.is_empty() {
            formatter.add_line(depth + 1, "Fields:");
            for field in &fragment.fields {
                let mut field_text = field.name.clone();
                if !field.directives.is_empty() {
                    let directive_strs: Vec<String> = field.directives.iter().map(|d| {
                        let emoji = match d.directive_type {
                            crate::extraction::graphql_parser::DirectiveType::Catch => "🧤",
                            crate::extraction::graphql_parser::DirectiveType::ThrowOnFieldError => "⚠️",
                        };
                        format!("{:?} {}", d.directive_type, emoji)
                    }).collect();
                    field_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                }
                formatter.add_line(depth + 2, &field_text);
            }
        }

        // Fragment children (nested fragments)
        if !fragment.children.is_empty() {
            formatter.add_line(depth + 1, "Children:");
            for child in &fragment.children {
                format_fragment_node(formatter, child, depth + 2, git_root);
            }
        }
    }

    /// Collects all TypeScript/TSX files from a fixture directory
    fn collect_fixture_files(dir_name: &str) -> Vec<String> {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("fixtures")
            .join(dir_name);

        let mut files = Vec::new();

        if let Ok(entries) = fs::read_dir(&fixture_dir) {
            let mut file_entries: Vec<_> = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    let path = entry.path();
                    path.is_file()
                        && (path.extension() == Some(std::ffi::OsStr::new("ts"))
                            || path.extension() == Some(std::ffi::OsStr::new("tsx")))
                })
                .collect();

            // Sort files by name for consistent ordering
            file_entries.sort_by_key(|entry| entry.file_name());

            for entry in file_entries {
                files.push(entry.path().to_string_lossy().to_string());
            }
        }

        files
    }

    /// Tests dependency graph building from valid fixture files
    #[test]
    fn test_dependency_graph_from_valid_fixtures() {
        let files = collect_fixture_files("valid");
        let registry = process_files(&files);
        let dependency_graph =
            registry_to_dependency_graph(&registry).expect("Failed to build dependency graph");
        let formatted = format_dependency_graph_with_tree_formatter(&dependency_graph);
        insta::assert_snapshot!(formatted);
    }

    /// Tests dependency graph building from invalid fixture files
    #[test]
    fn test_dependency_graph_from_invalid_fixtures() {
        let files = collect_fixture_files("invalid");
        let registry = process_files(&files);
        let dependency_graph =
            registry_to_dependency_graph(&registry).expect("Failed to build dependency graph");
        let formatted = format_dependency_graph_with_tree_formatter(&dependency_graph);
        insta::assert_snapshot!(formatted);
    }

    /// Tests dependency graph building from edge case fixture files
    #[test]
    fn test_dependency_graph_from_edge_case_fixtures() {
        let files = collect_fixture_files("edge_cases");
        let registry = process_files(&files);

        // Edge cases might include circular dependencies, so we handle the error case
        match registry_to_dependency_graph(&registry) {
            Ok(dependency_graph) => {
                let formatted = format_dependency_graph_with_tree_formatter(&dependency_graph);
                insta::assert_snapshot!(formatted);
            }
            Err(e) => {
                // For edge cases, we might get circular dependency errors
                // Still create a snapshot showing the error
                let error_output = format!("Dependency Graph Error: {}", e);
                insta::assert_snapshot!(error_output);
            }
        }
    }
}
