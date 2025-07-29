//! Concurrent GraphQL registry for fast file processing
//!
//! Uses DashMap for thread-safe concurrent access during parallel file parsing.
use anyhow::Result;
use dashmap::DashMap;
use globset::{Glob, GlobSetBuilder};
use ignore::{WalkBuilder, WalkState};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{atomic::AtomicUsize, Arc};

use crate::parsers::graphql_parser::{
    parse_graphql_to_ast, FragmentDefinition, GraphQLItem, QueryOperation,
};
use crate::parsers::typescript_parser::extract_graphql_from_file;

// Thread-safe storage for reusable GraphQL fragments
pub type FragmentRegistry = Arc<DashMap<String, FragmentDefinition>>;

// Thread-safe storage for main GraphQL operations
pub type QueryRegistry = Arc<DashMap<String, QueryOperation>>;

// Central store combining fragments and queries for validation
#[derive(Serialize, Deserialize)]
pub struct GraphQLRegistry {
    #[serde(with = "serde_dashmap")]
    pub fragments: FragmentRegistry,
    #[serde(with = "serde_dashmap")]
    pub queries: QueryRegistry,
    #[serde(skip)]
    pub file_count: usize,
}

// DashMap doesn't implement Serialize directly - need custom conversion
mod serde_dashmap {
    use super::*;
    use serde::{Deserializer, Serializer};
    use std::collections::HashMap;

    pub fn serialize<S, K, V>(
        dashmap: &Arc<DashMap<K, V>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        K: Clone + Serialize + Eq + std::hash::Hash,
        V: Clone + Serialize,
    {
        let map: HashMap<K, V> = dashmap
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
        map.serialize(serializer)
    }

    pub fn deserialize<'de, D, K, V>(deserializer: D) -> Result<Arc<DashMap<K, V>>, D::Error>
    where
        D: Deserializer<'de>,
        K: Clone + Deserialize<'de> + Eq + std::hash::Hash,
        V: Clone + Deserialize<'de>,
    {
        let map: HashMap<K, V> = HashMap::deserialize(deserializer)?;
        let dashmap = DashMap::new();
        for (k, v) in map {
            dashmap.insert(k, v);
        }
        Ok(Arc::new(dashmap))
    }
}

impl Default for GraphQLRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphQLRegistry {
    pub fn new() -> Self {
        Self {
            file_count: 0,
            fragments: Arc::new(DashMap::new()),
            queries: Arc::new(DashMap::new()),
        }
    }
}

// Parallel processing of file lists using rayon for performance
pub fn process_files(files: &[String]) -> GraphQLRegistry {
    let registry = GraphQLRegistry::new();

    files.par_iter().for_each(|file| {
        parse_file(Path::new(file), &registry);
    });

    registry
}

// Streaming approach avoids loading all files into memory at once
pub fn process_glob(
    root_path: &Path,
    include_patterns: &[&str], // e.g. &["**/*.ts", "**/*.tsx"]
    exclude_patterns: &[&str], // e.g. &["**/node_modules/**"]
) -> Result<GraphQLRegistry> {
    // Build include GlobSet
    let mut include_builder = GlobSetBuilder::new();
    for pattern in include_patterns {
        include_builder.add(Glob::new(pattern)?);
    }
    let include_set = Arc::new(include_builder.build()?);

    // Build exclude GlobSet
    let mut exclude_builder = GlobSetBuilder::new();
    for pattern in exclude_patterns {
        exclude_builder.add(Glob::new(pattern)?);
    }
    let exclude_set = Arc::new(exclude_builder.build()?);

    let mut registry = GraphQLRegistry::new();
    let registry_ref = &registry;
    let file_count = Arc::new(AtomicUsize::new(0));

    WalkBuilder::new(root_path)
        .standard_filters(false)
        .build_parallel()
        .run(|| {
            let include = Arc::clone(&include_set);
            let exclude = Arc::clone(&exclude_set);
            let registry = registry_ref;
            let file_counter = Arc::clone(&file_count);

            Box::new(move |entry_res: Result<ignore::DirEntry, ignore::Error>| {
                if let Ok(entry) = entry_res {
                    let path = entry.path();
                    if path.is_dir() && exclude.is_match(path) {
                        return WalkState::Skip;
                    } else if path.is_file() && include.is_match(path) {
                        parse_file(path, registry);
                        file_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                WalkState::Continue
            })
        });

    registry.file_count = file_count.load(std::sync::atomic::Ordering::Relaxed);

    Ok(registry)
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

    // Ensures consistent test output across different environments
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
                    let mut sorted_directives = query.directives.clone();
                    sorted_directives.sort_by(|a, b| {
                        format!("{:?}", a.directive_type).cmp(&format!("{:?}", b.directive_type))
                    });
                    for directive in &sorted_directives {
                        let emoji = match directive.directive_type {
                            crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                            crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError
                            | crate::parsers::graphql_parser::DirectiveType::RequiredThrow => "☄️",
                        };
                        formatter.add_line(4, &format!("{:?} {}", directive.directive_type, emoji));
                    }
                }

                // Query fields
                let mut query_fields = query.fields();
                if !query_fields.is_empty() {
                    query_fields.sort_by(|a, b| a.name.cmp(&b.name));
                    formatter.add_line(3, "Fields:");
                    for field in &query_fields {
                        let mut field_text = field.name.clone();
                        if !field.directives.is_empty() {
                            let mut sorted_field_directives = field.directives.clone();
                            sorted_field_directives.sort_by(|a, b| {
                                format!("{:?}", a.directive_type)
                                    .cmp(&format!("{:?}", b.directive_type))
                            });
                            let directive_strs: Vec<String> = sorted_field_directives.iter().map(|d| {
                                let emoji = match d.directive_type {
                                    crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                                    crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError |
                                    crate::parsers::graphql_parser::DirectiveType::RequiredThrow => "☄️",
                                };
                                format!("{:?} {}", d.directive_type, emoji)
                            }).collect();
                            field_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                        }
                        formatter.add_line(4, &field_text);
                    }
                }

                // Query fragment spreads
                let mut query_fragments = query.fragments();
                if !query_fragments.is_empty() {
                    query_fragments.sort_by(|a, b| a.name.cmp(&b.name));
                    formatter.add_line(3, "Fragment Spreads:");
                    for fragment in &query_fragments {
                        let mut fragment_text = fragment.name.clone();
                        if !fragment.directives.is_empty() {
                            let mut sorted_fragment_directives = fragment.directives.clone();
                            sorted_fragment_directives.sort_by(|a, b| {
                                format!("{:?}", a.directive_type)
                                    .cmp(&format!("{:?}", b.directive_type))
                            });
                            let directive_strs: Vec<String> = sorted_fragment_directives.iter().map(|d| {
                                let emoji = match d.directive_type {
                                    crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                                    crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError |
                                    crate::parsers::graphql_parser::DirectiveType::RequiredThrow => "☄️",
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
                    let mut sorted_directives = fragment.directives.clone();
                    sorted_directives.sort_by(|a, b| {
                        format!("{:?}", a.directive_type).cmp(&format!("{:?}", b.directive_type))
                    });
                    for directive in &sorted_directives {
                        let emoji = match directive.directive_type {
                            crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                            crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError
                            | crate::parsers::graphql_parser::DirectiveType::RequiredThrow => "☄️",
                        };
                        formatter.add_line(4, &format!("{:?} {}", directive.directive_type, emoji));
                    }
                }

                // Fragment fields
                let mut fragment_fields = fragment.fields();
                if !fragment_fields.is_empty() {
                    fragment_fields.sort_by(|a, b| a.name.cmp(&b.name));
                    formatter.add_line(3, "Fields:");
                    for field in &fragment_fields {
                        let mut field_text = field.name.clone();
                        if !field.directives.is_empty() {
                            let mut sorted_field_directives = field.directives.clone();
                            sorted_field_directives.sort_by(|a, b| {
                                format!("{:?}", a.directive_type)
                                    .cmp(&format!("{:?}", b.directive_type))
                            });
                            let directive_strs: Vec<String> = sorted_field_directives.iter().map(|d| {
                                let emoji = match d.directive_type {
                                    crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                                    crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError |
                                    crate::parsers::graphql_parser::DirectiveType::RequiredThrow => "☄️",
                                };
                                format!("{:?} {}", d.directive_type, emoji)
                            }).collect();
                            field_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                        }
                        formatter.add_line(4, &field_text);
                    }
                }

                // Fragment spreads
                let mut fragment_spreads = fragment.fragments();
                if !fragment_spreads.is_empty() {
                    fragment_spreads.sort_by(|a, b| a.name.cmp(&b.name));
                    formatter.add_line(3, "Fragment Spreads:");
                    for spread in &fragment_spreads {
                        let mut spread_text = spread.name.clone();
                        if !spread.directives.is_empty() {
                            let mut sorted_spread_directives = spread.directives.clone();
                            sorted_spread_directives.sort_by(|a, b| {
                                format!("{:?}", a.directive_type)
                                    .cmp(&format!("{:?}", b.directive_type))
                            });
                            let directive_strs: Vec<String> = sorted_spread_directives.iter().map(|d| {
                                let emoji = match d.directive_type {
                                    crate::parsers::graphql_parser::DirectiveType::Catch => "🧤",
                                    crate::parsers::graphql_parser::DirectiveType::ThrowOnFieldError |
                                    crate::parsers::graphql_parser::DirectiveType::RequiredThrow => "☄️",
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

    // Sorted file collection for deterministic test behavior
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

    // Validates that correct GraphQL is properly extracted
    #[test]
    fn test_registry_from_valid_fixtures() {
        let files = collect_fixture_files("valid");
        let registry = process_files(&files);
        let formatted = format_registry_with_tree_formatter(&registry);
        insta::assert_snapshot!(formatted);
    }

    // Ensures problematic GraphQL doesn't crash the parser
    #[test]
    fn test_registry_from_invalid_fixtures() {
        let files = collect_fixture_files("invalid");
        let registry = process_files(&files);
        let formatted = format_registry_with_tree_formatter(&registry);
        insta::assert_snapshot!(formatted);
    }

    // Covers complex scenarios like circular dependencies
    #[test]
    fn test_registry_from_edge_case_fixtures() {
        let files = collect_fixture_files("edge_cases");
        let registry = process_files(&files);
        let formatted = format_registry_with_tree_formatter(&registry);
        insta::assert_snapshot!(formatted);
    }
}
