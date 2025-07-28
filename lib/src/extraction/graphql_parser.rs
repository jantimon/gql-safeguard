//! GraphQL Parser and Directive Extraction
//!
//! This module provides functionality for parsing GraphQL strings into structured AST
//! representations and extracting GraphQL directives for analysis. It handles the
//! conversion from raw GraphQL template literals (extracted by `typescript_parser`)
//! into typed data structures suitable for directive validation and dependency analysis.
//!
//! # Architecture
//!
//! The parsing pipeline consists of two main phases:
//!
//! ## 1. GraphQL AST Parsing
//! - **Input**: Raw GraphQL strings from TypeScript template literals
//! - **Process**: Parse using `graphql-parser` crate for syntax validation
//! - **Output**: Structured `GraphQLItem` representations (queries and fragments)
//!
//! ## 2. Directive Extraction
//! - **Input**: Parsed GraphQL AST nodes
//! - **Process**: Extract `@catch` and `@throwOnFieldError` directives with locations
//! - **Output**: Typed directive data structures for validation analysis
//!
//! # Supported GraphQL Constructs
//!
//! - **Queries**: Named and anonymous query operations with field selections
//! - **Fragments**: Named fragment definitions with type conditions
//! - **Fragment Spreads**: References to other fragments via `...FragmentName`
//! - **Directives**: `@catch` and `@throwOnFieldError` directives on various locations
//! - **Fields**: Individual field selections with optional directives

use std::path::PathBuf;

use crate::extraction::typescript_parser::GraphQLString;
use anyhow::Result;
use graphql_parser::parse_query;
use graphql_parser::query::{
    Definition, Document as QueryDocument, OperationDefinition, Selection, SelectionSet,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectiveType {
    Catch,
    ThrowOnFieldError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Directive {
    pub directive_type: DirectiveType,
    pub position: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphQLItem {
    Query(QueryOperation),
    Fragment(FragmentDefinition),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryOperation {
    pub name: String,
    pub fields: Vec<Field>,
    pub fragments: Vec<FragmentSpread>,
    pub directives: Vec<Directive>,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentDefinition {
    pub name: String,
    pub fields: Vec<Field>,
    pub fragments: Vec<FragmentSpread>,
    pub directives: Vec<Directive>,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub directives: Vec<Directive>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentSpread {
    pub name: String,
    pub directives: Vec<Directive>,
}

/// Parses a GraphQL string into structured AST items with directive extraction.
///
/// This function takes a raw GraphQL string (extracted from TypeScript template literals)
/// and converts it into typed data structures representing queries, fragments, and their
/// associated directives. It performs syntax validation and extracts only the information
/// relevant for directive protection analysis
pub fn parse_graphql_to_ast(graphql_string: &GraphQLString) -> Result<Vec<GraphQLItem>> {
    // Parse the GraphQL string using the graphql-parser library
    let document: QueryDocument<String> = parse_query(&graphql_string.content).map_err(|e| {
        anyhow::anyhow!(
            "GraphQL syntax error in {} at position {}: {:?}",
            graphql_string.file_path.display(),
            graphql_string.position,
            e
        )
    })?;

    let mut items = Vec::new();

    // Process each definition in the GraphQL document
    for definition in document.definitions {
        match definition {
            Definition::Operation(op) => {
                if let Some(query) = convert_operation_to_query(
                    op,
                    &graphql_string.file_path,
                    graphql_string.position,
                )? {
                    items.push(GraphQLItem::Query(query));
                }
            }
            Definition::Fragment(frag) => {
                let fragment = convert_fragment_definition(
                    frag,
                    &graphql_string.file_path,
                    graphql_string.position,
                )?;
                items.push(GraphQLItem::Fragment(fragment));
            }
        }
    }

    Ok(items)
}

/// Converts a GraphQL operation definition to our internal QueryOperation representation.
///
/// This function processes query operations from the parsed GraphQL AST and extracts
/// the information needed for directive validation. It focuses on query operations
/// since mutations and subscriptions are not typically relevant for `@throwOnFieldError`
/// protection analysis
fn convert_operation_to_query(
    op: OperationDefinition<String>,
    file_path: &std::path::Path,
    position: u32,
) -> Result<Option<QueryOperation>> {
    match op {
        OperationDefinition::Query(query) => {
            // Assign a name to anonymous queries for tracking purposes
            let name = query.name.unwrap_or_else(|| "AnonymousQuery".to_string());

            // Extract directives from the query operation
            let directives = extract_directives_from_directive_list(&query.directives, position);

            // Process the selection set to extract fields and fragment spreads
            let (fields, fragments) =
                convert_selection_set_to_fields_and_spreads(&query.selection_set, position);

            Ok(Some(QueryOperation {
                name,
                fields,
                fragments,
                directives,
                file_path: file_path.to_path_buf(),
            }))
        }
        OperationDefinition::Mutation(_) | OperationDefinition::Subscription(_) => {
            // Skip mutations and subscriptions as they're not typically relevant
            // for @throwOnFieldError protection analysis
            Ok(None)
        }
        OperationDefinition::SelectionSet(_) => {
            // Skip bare selection sets for now - these are less common in practice
            // and would need additional handling for anonymous operation tracking
            Ok(None)
        }
    }
}

/// Converts a GraphQL fragment definition to our internal FragmentDefinition representation.
///
/// This function processes fragment definitions from the parsed GraphQL AST and extracts
/// all information needed for directive validation and dependency analysis. Fragments
/// are critical for protection analysis since `@catch` directives on fragments can
/// protect nested `@throwOnFieldError` directives
fn convert_fragment_definition(
    frag: graphql_parser::query::FragmentDefinition<String>,
    file_path: &std::path::Path,
    position: u32,
) -> Result<FragmentDefinition> {
    // Extract directives from the fragment definition
    let directives = extract_directives_from_directive_list(&frag.directives, position);

    // Process the selection set to extract fields and fragment spreads
    let (fields, fragments) =
        convert_selection_set_to_fields_and_spreads(&frag.selection_set, position);

    Ok(FragmentDefinition {
        name: frag.name,
        fields,
        fragments,
        directives,
        file_path: file_path.to_path_buf(),
    })
}

/// Converts a GraphQL selection set into separate lists of fields and fragment spreads.
///
/// This function processes the selection set from queries or fragments and separates
/// the different types of selections into appropriate data structures. This separation
/// is important for directive validation since protection rules apply differently to
/// direct field selections versus fragment spreads
fn convert_selection_set_to_fields_and_spreads(
    selection_set: &SelectionSet<String>,
    position: u32,
) -> (Vec<Field>, Vec<FragmentSpread>) {
    let mut fields = Vec::new();
    let mut fragments = Vec::new();

    for selection in &selection_set.items {
        match selection {
            Selection::Field(field) => {
                // Extract directives from the field selection
                let directives =
                    extract_directives_from_directive_list(&field.directives, position);
                fields.push(Field {
                    name: field.name.clone(),
                    directives,
                });

                // Recursively process nested selection set if it has items
                if !field.selection_set.items.is_empty() {
                    let (_nested_fields, nested_fragments) =
                        convert_selection_set_to_fields_and_spreads(&field.selection_set, position);
                    fragments.extend(nested_fragments);
                }
            }
            Selection::FragmentSpread(spread) => {
                // Extract directives from the fragment spread
                let directives =
                    extract_directives_from_directive_list(&spread.directives, position);
                fragments.push(FragmentSpread {
                    name: spread.fragment_name.clone(),
                    directives,
                });
            }
            Selection::InlineFragment(_) => {
                // Skip inline fragments for now - they're less common and would
                // require additional complexity for proper directive inheritance
            }
        }
    }

    (fields, fragments)
}

/// Extracts relevant GraphQL directives from a list of directive AST nodes.
///
/// This function processes the directive list from the GraphQL parser and converts
/// only the directives relevant to protection analysis (`@catch` and `@throwOnFieldError`)
/// into our internal `Directive` representation. Other directives are ignored since
/// they don't affect the safety analysis
fn extract_directives_from_directive_list(
    directives: &[graphql_parser::query::Directive<String>],
    position: u32,
) -> Vec<Directive> {
    directives
        .iter()
        .filter_map(|dir| {
            // Only process directives relevant to protection analysis
            let directive_type = match dir.name.as_str() {
                "catch" => DirectiveType::Catch,
                "throwOnFieldError" => DirectiveType::ThrowOnFieldError,
                _ => return None,
            };

            Some(Directive {
                directive_type,
                position,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extraction::typescript_parser;
    use std::fs;
    use std::path::PathBuf;

    /// Formats GraphQL AST items for snapshot testing with detailed structure visualization
    fn format_graphql_ast_result(
        file_path: &std::path::Path,
        graphql_items: &[GraphQLItem],
    ) -> String {
        // Convert to relative path from git root for portable snapshots
        let git_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let relative_path = file_path.strip_prefix(&git_root).unwrap_or(file_path);

        let mut result = format!("File: {}\n", relative_path.display());
        result.push_str(&format!("GraphQL AST items: {}\n\n", graphql_items.len()));

        for (i, item) in graphql_items.iter().enumerate() {
            result.push_str(&format!("=== AST Item {} ===\n", i + 1));

            match item {
                GraphQLItem::Query(query) => {
                    result.push_str(&format!("Type: Query\n"));
                    result.push_str(&format!("Name: {}\n", query.name));

                    // Use relative path for file location in AST items too
                    let query_file_relative = query
                        .file_path
                        .strip_prefix(&git_root)
                        .unwrap_or(&query.file_path);
                    result.push_str(&format!("File: {}\n", query_file_relative.display()));

                    // Show query-level directives with emojis
                    result.push_str(&format!("Directives: {}\n", query.directives.len()));
                    for directive in &query.directives {
                        let emoji = match directive.directive_type {
                            DirectiveType::Catch => "🧤",
                            DirectiveType::ThrowOnFieldError => "☄️",
                        };
                        result.push_str(&format!("  - {:?} {}\n", directive.directive_type, emoji));
                    }

                    // Show fields with their directives
                    result.push_str(&format!("Fields: {}\n", query.fields.len()));
                    for field in &query.fields {
                        result.push_str(&format!("  - {}", field.name));
                        if !field.directives.is_empty() {
                            result.push_str(" [");
                            for (j, directive) in field.directives.iter().enumerate() {
                                if j > 0 {
                                    result.push_str(", ");
                                }
                                let emoji = match directive.directive_type {
                                    DirectiveType::Catch => "🧤",
                                    DirectiveType::ThrowOnFieldError => "☄️",
                                };
                                result
                                    .push_str(&format!("{:?} {}", directive.directive_type, emoji));
                            }
                            result.push_str("]");
                        }
                        result.push_str("\n");
                    }

                    // Show fragment spreads
                    result.push_str(&format!("Fragment Spreads: {}\n", query.fragments.len()));
                    for fragment in &query.fragments {
                        result.push_str(&format!("  - {}", fragment.name));
                        if !fragment.directives.is_empty() {
                            result.push_str(" [");
                            for (j, directive) in fragment.directives.iter().enumerate() {
                                if j > 0 {
                                    result.push_str(", ");
                                }
                                let emoji = match directive.directive_type {
                                    DirectiveType::Catch => "🧤",
                                    DirectiveType::ThrowOnFieldError => "☄️",
                                };
                                result
                                    .push_str(&format!("{:?} {}", directive.directive_type, emoji));
                            }
                            result.push_str("]");
                        }
                        result.push_str("\n");
                    }
                }
                GraphQLItem::Fragment(fragment) => {
                    result.push_str(&format!("Type: Fragment\n"));
                    result.push_str(&format!("Name: {}\n", fragment.name));

                    // Use relative path for file location in AST items too
                    let fragment_file_relative = fragment
                        .file_path
                        .strip_prefix(&git_root)
                        .unwrap_or(&fragment.file_path);
                    result.push_str(&format!("File: {}\n", fragment_file_relative.display()));

                    // Show fragment-level directives with emojis
                    result.push_str(&format!("Directives: {}\n", fragment.directives.len()));
                    for directive in &fragment.directives {
                        let emoji = match directive.directive_type {
                            DirectiveType::Catch => "🧤",
                            DirectiveType::ThrowOnFieldError => "☄️",
                        };
                        result.push_str(&format!("  - {:?} {}\n", directive.directive_type, emoji));
                    }

                    // Show fields with their directives
                    result.push_str(&format!("Fields: {}\n", fragment.fields.len()));
                    for field in &fragment.fields {
                        result.push_str(&format!("  - {}", field.name));
                        if !field.directives.is_empty() {
                            result.push_str(" [");
                            for (j, directive) in field.directives.iter().enumerate() {
                                if j > 0 {
                                    result.push_str(", ");
                                }
                                let emoji = match directive.directive_type {
                                    DirectiveType::Catch => "🧤",
                                    DirectiveType::ThrowOnFieldError => "☄️",
                                };
                                result
                                    .push_str(&format!("{:?} {}", directive.directive_type, emoji));
                            }
                            result.push_str("]");
                        }
                        result.push_str("\n");
                    }

                    // Show fragment spreads
                    result.push_str(&format!("Fragment Spreads: {}\n", fragment.fragments.len()));
                    for spread in &fragment.fragments {
                        result.push_str(&format!("  - {}", spread.name));
                        if !spread.directives.is_empty() {
                            result.push_str(" [");
                            for (j, directive) in spread.directives.iter().enumerate() {
                                if j > 0 {
                                    result.push_str(", ");
                                }
                                let emoji = match directive.directive_type {
                                    DirectiveType::Catch => "🧤",
                                    DirectiveType::ThrowOnFieldError => "☄️",
                                };
                                result
                                    .push_str(&format!("{:?} {}", directive.directive_type, emoji));
                            }
                            result.push_str("]");
                        }
                        result.push_str("\n");
                    }
                }
            }
            result.push_str("\n");
        }

        result
    }

    /// Processes all GraphQL strings from a fixture directory and parses them to AST
    fn process_fixture_directory_to_ast(dir_name: &str) -> String {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("fixtures")
            .join(dir_name);

        let mut results = Vec::new();

        if let Ok(entries) = fs::read_dir(&fixture_dir) {
            let mut files: Vec<_> = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    let path = entry.path();
                    path.is_file()
                        && (path.extension() == Some(std::ffi::OsStr::new("ts"))
                            || path.extension() == Some(std::ffi::OsStr::new("tsx")))
                })
                .collect();

            // Sort files by name for consistent snapshot ordering
            files.sort_by_key(|entry| entry.file_name());

            for entry in files {
                let file_path = entry.path();

                // First extract GraphQL strings from TypeScript
                match typescript_parser::extract_graphql_from_file(&file_path) {
                    Ok(graphql_strings) => {
                        // Then parse each GraphQL string to AST
                        for graphql_string in graphql_strings {
                            match parse_graphql_to_ast(&graphql_string) {
                                Ok(graphql_items) => {
                                    let result =
                                        format_graphql_ast_result(&file_path, &graphql_items);
                                    results.push(result);
                                }
                                Err(e) => {
                                    results.push(format!(
                                        "File: {}\nGraphQL Parse Error: {}\nContent: {}\n\n",
                                        file_path.display(),
                                        e,
                                        graphql_string.content
                                    ));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        results.push(format!(
                            "File: {}\nTypeScript Parse Error: {}\n\n",
                            file_path.display(),
                            e
                        ));
                    }
                }
            }
        }

        results.join("---\n\n")
    }

    /// Tests GraphQL AST parsing from all valid fixture files
    #[test]
    fn test_parse_valid_fixtures_to_ast() {
        let result = process_fixture_directory_to_ast("valid");
        insta::assert_snapshot!(result);
    }

    /// Tests GraphQL AST parsing from all invalid fixture files
    #[test]
    fn test_parse_invalid_fixtures_to_ast() {
        let result = process_fixture_directory_to_ast("invalid");
        insta::assert_snapshot!(result);
    }

    /// Tests GraphQL AST parsing from all edge case fixture files
    #[test]
    fn test_parse_edge_case_fixtures_to_ast() {
        let result = process_fixture_directory_to_ast("edge_cases");
        insta::assert_snapshot!(result);
    }
}
