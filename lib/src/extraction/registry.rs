//! GraphQL Extraction Registry
//!

/// A dashmap registry with:
/// process_files()
///
/// a dashmap regisry for all fragments (key fragment name)
/// a dashmap registry for all queries (key query name)
use dashmap::DashMap;
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;

use crate::extraction::graphql_parser::{
    parse_graphql_to_ast, FragmentDefinition, GraphQLItem, QueryOperation,
};
use crate::extraction::typescript_parser::extract_graphql_from_file;

/// Registry for GraphQL fragments
pub type FragmentRegistry = Arc<DashMap<String, FragmentDefinition>>;

/// Registry for GraphQL queries
pub type QueryRegistry = Arc<DashMap<String, QueryOperation>>;

/// Main registry that holds both fragments and queries
pub struct GraphQLRegistry {
    pub fragments: FragmentRegistry,
    pub queries: QueryRegistry,
}

impl Default for GraphQLRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphQLRegistry {
    pub fn new() -> Self {
        Self {
            fragments: Arc::new(DashMap::new()),
            queries: Arc::new(DashMap::new()),
        }
    }
}

/// Function which returns a new GraphQL registry for the given file list
pub fn process_files(files: &[String]) -> GraphQLRegistry {
    let registry = GraphQLRegistry::new();

    files.par_iter().for_each(|file| {
        parse_file(Path::new(file), &registry);
    });

    registry
}

fn parse_file(file: &Path, registry: &GraphQLRegistry) {
    let graphql_strings_result = extract_graphql_from_file(Path::new(file));
    if let Ok(graphql_strings) = graphql_strings_result {
        for graphql_string in &graphql_strings {
            let graphql_ast = parse_graphql_to_ast(graphql_string);
            if let Ok(ast) = graphql_ast {
                for graphql_item in ast {
                    match graphql_item {
                        GraphQLItem::Fragment(fragment) => {
                            registry.fragments.insert(fragment.name.clone(), fragment);
                        }
                        GraphQLItem::Query(query) => {
                            registry.queries.insert(query.name.clone(), query);
                        }
                    }
                }
            }
        }
    } else {
        eprintln!("Failed to parse GraphQL from file: {}", file.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree_formatter::TreeFormatter;
    use std::fs;
    use std::path::PathBuf;

    /// Formats a GraphQL registry using TreeFormatter for snapshot testing
    fn format_registry_with_tree_formatter(registry: &GraphQLRegistry) -> String {
        let mut formatter = TreeFormatter::new();

        // Git root for relative paths in snapshots
        let git_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();

        formatter.add_line(0, "GraphQL Registry");

        // Format queries section
        let query_count = registry.queries.len();
        formatter.add_line(1, &format!("Queries ({})", query_count));

        if query_count > 0 {
            // Sort queries by name for consistent output
            let mut queries: Vec<_> = registry.queries.iter().collect();
            queries.sort_by(|a, b| a.key().cmp(b.key()));

            for entry in queries {
                let name = entry.key();
                let query = entry.value();
                let relative_path = query
                    .file_path
                    .strip_prefix(&git_root)
                    .unwrap_or(&query.file_path);
                formatter.add_line(2, &format!("{} ({})", name, relative_path.display()));

                // Query directives
                if !query.directives.is_empty() {
                    formatter.add_line(3, "Directives:");
                    for directive in &query.directives {
                        let emoji = match directive.directive_type {
                            crate::extraction::graphql_parser::DirectiveType::Catch => "🧤",
                            crate::extraction::graphql_parser::DirectiveType::ThrowOnFieldError => {
                                "⚠️"
                            }
                        };
                        formatter.add_line(4, &format!("{:?} {}", directive.directive_type, emoji));
                    }
                }

                // Query fields
                if !query.fields.is_empty() {
                    formatter.add_line(3, "Fields:");
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
                        formatter.add_line(4, &field_text);
                    }
                }

                // Query fragment spreads
                if !query.fragments.is_empty() {
                    formatter.add_line(3, "Fragment Spreads:");
                    for fragment in &query.fragments {
                        let mut fragment_text = fragment.name.clone();
                        if !fragment.directives.is_empty() {
                            let directive_strs: Vec<String> = fragment.directives.iter().map(|d| {
                                let emoji = match d.directive_type {
                                    crate::extraction::graphql_parser::DirectiveType::Catch => "🧤",
                                    crate::extraction::graphql_parser::DirectiveType::ThrowOnFieldError => "⚠️",
                                };
                                format!("{:?} {}", d.directive_type, emoji)
                            }).collect();
                            fragment_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                        }
                        formatter.add_line(4, &fragment_text);
                    }
                }
            }
        }

        // Format fragments section
        let fragment_count = registry.fragments.len();
        formatter.add_line(1, &format!("Fragments ({})", fragment_count));

        if fragment_count > 0 {
            // Sort fragments by name for consistent output
            let mut fragments: Vec<_> = registry.fragments.iter().collect();
            fragments.sort_by(|a, b| a.key().cmp(b.key()));

            for entry in fragments {
                let name = entry.key();
                let fragment = entry.value();
                let relative_path = fragment
                    .file_path
                    .strip_prefix(&git_root)
                    .unwrap_or(&fragment.file_path);
                formatter.add_line(2, &format!("{} ({})", name, relative_path.display()));

                // Fragment directives
                if !fragment.directives.is_empty() {
                    formatter.add_line(3, "Directives:");
                    for directive in &fragment.directives {
                        let emoji = match directive.directive_type {
                            crate::extraction::graphql_parser::DirectiveType::Catch => "🧤",
                            crate::extraction::graphql_parser::DirectiveType::ThrowOnFieldError => {
                                "⚠️"
                            }
                        };
                        formatter.add_line(4, &format!("{:?} {}", directive.directive_type, emoji));
                    }
                }

                // Fragment fields
                if !fragment.fields.is_empty() {
                    formatter.add_line(3, "Fields:");
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
                        formatter.add_line(4, &field_text);
                    }
                }

                // Fragment spreads
                if !fragment.fragments.is_empty() {
                    formatter.add_line(3, "Fragment Spreads:");
                    for spread in &fragment.fragments {
                        let mut spread_text = spread.name.clone();
                        if !spread.directives.is_empty() {
                            let directive_strs: Vec<String> = spread.directives.iter().map(|d| {
                                let emoji = match d.directive_type {
                                    crate::extraction::graphql_parser::DirectiveType::Catch => "🧤",
                                    crate::extraction::graphql_parser::DirectiveType::ThrowOnFieldError => "⚠️",
                                };
                                format!("{:?} {}", d.directive_type, emoji)
                            }).collect();
                            spread_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                        }
                        formatter.add_line(4, &spread_text);
                    }
                }
            }
        }

        formatter.to_string()
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

    /// Tests registry building from valid fixture files
    #[test]
    fn test_registry_from_valid_fixtures() {
        let files = collect_fixture_files("valid");
        let registry = process_files(&files);
        let formatted = format_registry_with_tree_formatter(&registry);
        insta::assert_snapshot!(formatted);
    }

    /// Tests registry building from invalid fixture files
    #[test]
    fn test_registry_from_invalid_fixtures() {
        let files = collect_fixture_files("invalid");
        let registry = process_files(&files);
        let formatted = format_registry_with_tree_formatter(&registry);
        insta::assert_snapshot!(formatted);
    }

    /// Tests registry building from edge case fixture files
    #[test]
    fn test_registry_from_edge_case_fixtures() {
        let files = collect_fixture_files("edge_cases");
        let registry = process_files(&files);
        let formatted = format_registry_with_tree_formatter(&registry);
        insta::assert_snapshot!(formatted);
    }

    /// Tests registry building from all fixture files combined
    #[test]
    fn test_registry_from_all_fixtures() {
        let mut all_files = Vec::new();
        all_files.extend(collect_fixture_files("valid"));
        all_files.extend(collect_fixture_files("invalid"));
        all_files.extend(collect_fixture_files("edge_cases"));

        let registry = process_files(&all_files);
        let formatted = format_registry_with_tree_formatter(&registry);
        insta::assert_snapshot!(formatted);
    }
}
