//! Registry to Dependency Graph Conversion
//!
//! This module transforms a GraphQL registry into a hierarchical dependency graph structure
//! where queries contain their fragment dependencies as nested trees. This enables easier
//! analysis of directive protection inheritance and dependency relationships.

use anyhow::{Context, Result};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::parsers::graphql_parser::{Directive, Selection};
use crate::registry::GraphQLRegistry;

/// A query with its complete fragment dependency tree resolved
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryWithFragments {
    pub name: String,
    pub file_path: PathBuf,
    pub directives: Vec<Directive>,
    pub selections: Vec<Selection>, // Now hierarchical
}

/// A fragment node in the dependency tree with its children recursively resolved
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentNode {
    pub name: String,
    pub file_path: PathBuf,
    pub directives: Vec<Directive>,
    pub selections: Vec<Selection>, // Now hierarchical
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

        // Build hierarchical selections with resolved fragments
        let resolved_selections = resolve_selections_with_fragments(
            &query.selections,
            registry,
            &mut FxHashSet::default(),
        )
        .with_context(|| format!("Failed to resolve selections for query '{}'", query_name))?;

        result.push(QueryWithFragments {
            name: query.name.clone(),
            file_path: query.file_path.clone(),
            directives: query.directives.clone(),
            selections: resolved_selections,
        });
    }

    // Sort by query name for consistent output
    result.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(result)
}

/// Recursively resolves selections with fragment dependencies in hierarchical structure
///
/// This function processes a list of selections and resolves any fragment spreads
/// by substituting them with their actual definitions while preserving the
/// hierarchical nesting structure. It maintains a visiting set to detect circular dependencies.
///
/// # Arguments
/// * `selections` - The selections to resolve (fields, fragment spreads, inline fragments)
/// * `registry` - The GraphQL registry containing fragment definitions
/// * `visiting` - Set of fragment names currently being visited (for cycle detection)
///
/// # Returns
/// * `Vec<Selection>` - The resolved hierarchical selections with fragments expanded
///
/// # Errors
/// * Returns error if circular dependency is detected
/// * Returns error if a fragment spread references a non-existent fragment
fn resolve_selections_with_fragments(
    selections: &[Selection],
    registry: &GraphQLRegistry,
    visiting: &mut FxHashSet<String>,
) -> Result<Vec<Selection>> {
    let mut resolved_selections = Vec::new();

    for selection in selections {
        match selection {
            Selection::Field(field_selection) => {
                // Recursively resolve nested selections in the field
                let resolved_nested = resolve_selections_with_fragments(
                    &field_selection.selections,
                    registry,
                    visiting,
                )?;

                resolved_selections.push(Selection::Field(
                    crate::parsers::graphql_parser::FieldSelection {
                        name: field_selection.name.clone(),
                        directives: field_selection.directives.clone(),
                        selections: resolved_nested,
                    },
                ));
            }
            Selection::FragmentSpread(spread) => {
                // Resolve the fragment spread by replacing it with the fragment's content
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

                // Recursively resolve the fragment's selections
                let resolved_fragment_selections =
                    resolve_selections_with_fragments(&fragment.selections, registry, visiting)?;

                // Remove from visiting set (backtrack)
                visiting.remove(&spread.name);

                // Create a fragment node that contains the resolved selections
                // This preserves the fragment boundary while expanding its content
                resolved_selections.push(Selection::InlineFragment(
                    crate::parsers::graphql_parser::InlineFragment {
                        type_condition: Some(format!("{}Fragment", spread.name)), // Mark as resolved fragment
                        directives: {
                            let mut combined_directives = spread.directives.clone();
                            combined_directives.extend(fragment.directives.clone());
                            combined_directives
                        },
                        selections: resolved_fragment_selections,
                    },
                ));
            }
            Selection::InlineFragment(inline) => {
                // Recursively resolve nested selections in the inline fragment
                let resolved_nested =
                    resolve_selections_with_fragments(&inline.selections, registry, visiting)?;

                resolved_selections.push(Selection::InlineFragment(
                    crate::parsers::graphql_parser::InlineFragment {
                        type_condition: inline.type_condition.clone(),
                        directives: inline.directives.clone(),
                        selections: resolved_nested,
                    },
                ));
            }
        }
    }

    Ok(resolved_selections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::process_files;
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
                        crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                        crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError => "⚠️",
                    };
                    formatter.add_line(3, &format!("{:?} {}", directive.directive_type, emoji));
                }
            }

            // Query selections (hierarchical)
            if !query.selections.is_empty() {
                formatter.add_line(2, "Selections:");
                format_selections(&mut formatter, &query.selections, 3, &git_root);
            }
        }

        formatter.to_string()
    }

    /// Recursively formats hierarchical selections preserving nesting structure
    fn format_selections(
        formatter: &mut TreeFormatter,
        selections: &[Selection],
        depth: usize,
        git_root: &PathBuf,
    ) {
        for selection in selections {
            match selection {
                Selection::Field(field_selection) => {
                    // Format field with its directives
                    let mut field_text = format!("Field: {}", field_selection.name);
                    if !field_selection.directives.is_empty() {
                        let directive_strs: Vec<String> = field_selection.directives.iter().map(|d| {
                            let emoji = match d.directive_type {
                                crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                                crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError => "⚠️",
                            };
                            format!("{:?} {}", d.directive_type, emoji)
                        }).collect();
                        field_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                    }
                    formatter.add_line(depth, &field_text);

                    // Recursively format nested selections with increased depth
                    if !field_selection.selections.is_empty() {
                        format_selections(
                            formatter,
                            &field_selection.selections,
                            depth + 1,
                            git_root,
                        );
                    }
                }
                Selection::FragmentSpread(spread) => {
                    // Format fragment spread with its directives
                    let mut spread_text = format!("FragmentSpread: {}", spread.name);
                    if !spread.directives.is_empty() {
                        let directive_strs: Vec<String> = spread.directives.iter().map(|d| {
                            let emoji = match d.directive_type {
                                crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                                crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError => "⚠️",
                            };
                            format!("{:?} {}", d.directive_type, emoji)
                        }).collect();
                        spread_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                    }
                    formatter.add_line(depth, &spread_text);
                }
                Selection::InlineFragment(inline) => {
                    // Format inline fragment (this represents resolved fragments)
                    let mut inline_text = "ResolvedFragment:".to_string();
                    if let Some(type_condition) = &inline.type_condition {
                        if type_condition.ends_with("Fragment") {
                            // Extract original fragment name
                            let fragment_name = type_condition
                                .strip_suffix("Fragment")
                                .unwrap_or(type_condition);
                            inline_text = format!("Fragment: {}", fragment_name);
                        }
                    }

                    if !inline.directives.is_empty() {
                        let directive_strs: Vec<String> = inline.directives.iter().map(|d| {
                            let emoji = match d.directive_type {
                                crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                                crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError => "⚠️",
                            };
                            format!("{:?} {}", d.directive_type, emoji)
                        }).collect();
                        inline_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                    }
                    formatter.add_line(depth, &inline_text);

                    // Recursively format the fragment's content with increased depth
                    if !inline.selections.is_empty() {
                        format_selections(formatter, &inline.selections, depth + 1, git_root);
                    }
                }
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
