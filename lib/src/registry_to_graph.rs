//! Converts flat fragment registry into resolved dependency trees
//!
//! Enables validation by expanding fragment spreads into complete query structures
//! while preserving directive inheritance and protection relationships.

use anyhow::{Context, Result};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::parsers::graphql_parser::{Directive, Selection};
use crate::registry::GraphQLRegistry;

// Query with all fragment spreads replaced by actual fragment content
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryWithFragments {
    pub name: String,
    pub file_path: PathBuf,
    pub directives: Vec<Directive>,
    pub selections: Vec<Selection>, // Now hierarchical
}

// Fragment with all nested dependencies expanded for validation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentNode {
    pub name: String,
    pub file_path: PathBuf,
    pub directives: Vec<Directive>,
    pub selections: Vec<Selection>, // Now hierarchical
}

// Main entry point: expands all fragment dependencies for validation
// Transforms queries from flat registry into complete hierarchical structures
pub fn registry_to_dependency_graph(registry: &GraphQLRegistry) -> Result<Vec<QueryWithFragments>> {
    let mut result = Vec::new();

    // Process each query in the registry
    for query_entry in registry.queries.iter() {
        let query_name = query_entry.key();
        let query = query_entry.value();

        // Expand fragment spreads into complete dependency tree
        let resolved_selections = resolve_selections_with_fragments(
            &query.selections,
            registry,
            &mut FxHashSet::default(),
        )
        .with_context(|| format!("Failed to resolve selections for query '{query_name}'"))?;

        result.push(QueryWithFragments {
            name: query.name.clone(),
            file_path: query.file_path.clone(),
            directives: query.directives.clone(),
            selections: resolved_selections,
        });
    }

    // Deterministic ordering for reliable snapshots and diffs
    result.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(result)
}

// Core resolution algorithm: replaces fragment spreads with actual fragment content
// Uses cycle detection to prevent infinite recursion from circular dependencies
fn resolve_selections_with_fragments(
    selections: &[Selection],
    registry: &GraphQLRegistry,
    visiting: &mut FxHashSet<String>,
) -> Result<Vec<Selection>> {
    let mut resolved_selections = Vec::new();

    for selection in selections {
        match selection {
            Selection::Field(field_selection) => {
                // Fields may contain fragment spreads that need resolution
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
                // Replace ...FragmentName with actual fragment selections
                let fragment_entry = registry
                    .fragments
                    .get(&spread.name)
                    .with_context(|| format!("Fragment '{}' not found in registry", spread.name))?;

                let fragment = fragment_entry.value();

                // Prevent infinite recursion from fragment cycles
                if visiting.contains(&spread.name) {
                    return Err(anyhow::anyhow!(
                        "Circular dependency detected involving fragment '{}'",
                        spread.name
                    ));
                }

                // Track current path to detect cycles
                visiting.insert(spread.name.clone());

                // Fragment may contain other fragments needing expansion
                let resolved_fragment_selections =
                    resolve_selections_with_fragments(&fragment.selections, registry, visiting)?;

                // Clean up cycle detection after processing
                visiting.remove(&spread.name);

                // Preserve fragment identity while expanding its content for validation
                resolved_selections.push(Selection::InlineFragment(
                    crate::parsers::graphql_parser::InlineFragment {
                        type_condition: Some(format!("{}Fragment", spread.name)), // Distinguish resolved fragments
                        directives: {
                            // Merge spread and fragment directives for protection inheritance
                            let mut combined_directives = spread.directives.clone();
                            combined_directives.extend(fragment.directives.clone());
                            combined_directives
                        },
                        selections: resolved_fragment_selections,
                    },
                ));
            }
            Selection::InlineFragment(inline) => {
                // Inline fragments may also contain spreads needing resolution
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

    // Consistent test output format for regression testing
    fn format_dependency_graph_with_tree_formatter(queries: &[QueryWithFragments]) -> String {
        let mut formatter = TreeFormatter::new();

        // Portable paths across different development environments
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
                        crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError
                        | crate::parsers::graphql_parser::DirectiveType::RequiredThrow => "☄️",
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

    // Visual representation maintains query structure hierarchy
    fn format_selections(
        formatter: &mut TreeFormatter,
        selections: &[Selection],
        depth: usize,
        git_root: &PathBuf,
    ) {
        for selection in selections {
            match selection {
                Selection::Field(field_selection) => {
                    // Show field-level protection directives
                    let mut field_text = format!("Field: {}", field_selection.name);
                    if !field_selection.directives.is_empty() {
                        let directive_strs: Vec<String> = field_selection.directives.iter().map(|d| {
                            let emoji = match d.directive_type {
                                crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                                crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError |
                                crate::parsers::graphql_parser::DirectiveType::RequiredThrow => "☄️",
                            };
                            format!("{:?} {}", d.directive_type, emoji)
                        }).collect();
                        field_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                    }
                    formatter.add_line(depth, &field_text);

                    // Maintain visual hierarchy for nested structures
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
                    // Show spread-level directives before expansion
                    let mut spread_text = format!("FragmentSpread: {}", spread.name);
                    if !spread.directives.is_empty() {
                        let directive_strs: Vec<String> = spread.directives.iter().map(|d| {
                            let emoji = match d.directive_type {
                                crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                                crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError |
                                crate::parsers::graphql_parser::DirectiveType::RequiredThrow => "☄️",
                            };
                            format!("{:?} {}", d.directive_type, emoji)
                        }).collect();
                        spread_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                    }
                    formatter.add_line(depth, &spread_text);
                }
                Selection::InlineFragment(inline) => {
                    // Show expanded fragment content with preserved identity
                    let mut inline_text = "ResolvedFragment:".to_string();
                    if let Some(type_condition) = &inline.type_condition {
                        if type_condition.ends_with("Fragment") {
                            // Restore readable fragment name for display
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
                                crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError |
                                crate::parsers::graphql_parser::DirectiveType::RequiredThrow => "☄️",
                            };
                            format!("{:?} {}", d.directive_type, emoji)
                        }).collect();
                        inline_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                    }
                    formatter.add_line(depth, &inline_text);

                    // Show expanded fragment structure
                    if !inline.selections.is_empty() {
                        format_selections(formatter, &inline.selections, depth + 1, git_root);
                    }
                }
            }
        }
    }

    // Deterministic file ordering for consistent test snapshots
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

            // Prevents flaky tests from filesystem ordering differences
            file_entries.sort_by_key(|entry| entry.file_name());

            for entry in file_entries {
                files.push(entry.path().to_string_lossy().to_string());
            }
        }

        files
    }

    // Validates correct fragment resolution for well-formed GraphQL
    #[test]
    fn test_dependency_graph_from_valid_fixtures() {
        let files = collect_fixture_files("valid");
        let registry = process_files(&files);
        let dependency_graph =
            registry_to_dependency_graph(&registry).expect("Failed to build dependency graph");
        let formatted = format_dependency_graph_with_tree_formatter(&dependency_graph);
        insta::assert_snapshot!(formatted);
    }

    // Ensures invalid GraphQL still produces parseable structures
    #[test]
    fn test_dependency_graph_from_invalid_fixtures() {
        let files = collect_fixture_files("invalid");
        let registry = process_files(&files);
        let dependency_graph =
            registry_to_dependency_graph(&registry).expect("Failed to build dependency graph");
        let formatted = format_dependency_graph_with_tree_formatter(&dependency_graph);
        insta::assert_snapshot!(formatted);
    }

    // Handles complex scenarios like circular dependencies gracefully
    #[test]
    fn test_dependency_graph_from_edge_case_fixtures() {
        let files = collect_fixture_files("edge_cases");
        let registry = process_files(&files);

        // Circular dependencies should fail gracefully with clear error messages
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
