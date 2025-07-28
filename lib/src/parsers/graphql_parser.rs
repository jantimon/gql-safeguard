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

use crate::parsers::typescript_parser::GraphQLString;
use anyhow::Result;
use graphql_parser::parse_query;
use graphql_parser::query::{
    Definition, Document as QueryDocument, OperationDefinition, SelectionSet,
};
use serde::{Deserialize, Serialize};

/// Legacy flat field structure for compatibility
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub directives: Vec<Directive>,
}

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
pub enum Selection {
    Field(FieldSelection),
    FragmentSpread(FragmentSpread),
    InlineFragment(InlineFragment),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldSelection {
    pub name: String,
    pub directives: Vec<Directive>,
    pub selections: Vec<Selection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentSpread {
    pub name: String,
    pub directives: Vec<Directive>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineFragment {
    pub type_condition: Option<String>,
    pub directives: Vec<Directive>,
    pub selections: Vec<Selection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryOperation {
    pub name: String,
    pub selections: Vec<Selection>,
    pub directives: Vec<Directive>,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentDefinition {
    pub name: String,
    pub type_condition: String,
    pub selections: Vec<Selection>,
    pub directives: Vec<Directive>,
    pub file_path: PathBuf,
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

            // Process the selection set to build hierarchical structure
            let selections = convert_selection_set(&query.selection_set, position);

            Ok(Some(QueryOperation {
                name,
                selections,
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

    // Process the selection set to build hierarchical structure
    let selections = convert_selection_set(&frag.selection_set, position);

    Ok(FragmentDefinition {
        name: frag.name,
        type_condition: frag.type_condition.to_string(),
        selections,
        directives,
        file_path: file_path.to_path_buf(),
    })
}

/// Converts a GraphQL selection set into hierarchical Selection structures.
///
/// This function processes the selection set from queries or fragments and builds
/// a hierarchical structure that preserves the nesting relationships between
/// fields and fragments. This is crucial for directive validation since
/// protection rules need to understand the parent-child relationships.
fn convert_selection_set(selection_set: &SelectionSet<String>, position: u32) -> Vec<Selection> {
    let mut selections = Vec::new();

    for selection in &selection_set.items {
        match selection {
            graphql_parser::query::Selection::Field(field) => {
                // Extract directives from the field selection
                let directives =
                    extract_directives_from_directive_list(&field.directives, position);

                // Recursively process nested selection set
                let nested_selections = convert_selection_set(&field.selection_set, position);

                selections.push(Selection::Field(FieldSelection {
                    name: field.name.clone(),
                    directives,
                    selections: nested_selections,
                }));
            }
            graphql_parser::query::Selection::FragmentSpread(spread) => {
                // Extract directives from the fragment spread
                let directives =
                    extract_directives_from_directive_list(&spread.directives, position);

                selections.push(Selection::FragmentSpread(FragmentSpread {
                    name: spread.fragment_name.clone(),
                    directives,
                }));
            }
            graphql_parser::query::Selection::InlineFragment(inline) => {
                // Extract directives from the inline fragment
                let directives =
                    extract_directives_from_directive_list(&inline.directives, position);

                // Recursively process nested selection set
                let nested_selections = convert_selection_set(&inline.selection_set, position);

                selections.push(Selection::InlineFragment(InlineFragment {
                    type_condition: inline.type_condition.as_ref().map(|tc| tc.to_string()),
                    directives,
                    selections: nested_selections,
                }));
            }
        }
    }

    selections
}

/// Helper functions for backward compatibility with modules expecting flat structures
impl QueryOperation {
    /// Extract all fields from the hierarchical selection structure (flattened)
    pub fn fields(&self) -> Vec<Field> {
        extract_fields_from_selections(&self.selections)
    }

    /// Extract all fragment spreads from the hierarchical selection structure (flattened)
    pub fn fragments(&self) -> Vec<FragmentSpread> {
        extract_fragment_spreads_from_selections(&self.selections)
    }
}

impl FragmentDefinition {
    /// Extract all fields from the hierarchical selection structure (flattened)
    pub fn fields(&self) -> Vec<Field> {
        extract_fields_from_selections(&self.selections)
    }

    /// Extract all fragment spreads from the hierarchical selection structure (flattened)
    pub fn fragments(&self) -> Vec<FragmentSpread> {
        extract_fragment_spreads_from_selections(&self.selections)
    }
}

/// Recursively extract all fields from a selection hierarchy (flattened)
fn extract_fields_from_selections(selections: &[Selection]) -> Vec<Field> {
    let mut fields = Vec::new();

    for selection in selections {
        match selection {
            Selection::Field(field_selection) => {
                fields.push(Field {
                    name: field_selection.name.clone(),
                    directives: field_selection.directives.clone(),
                });
                // Recursively extract nested fields
                fields.extend(extract_fields_from_selections(&field_selection.selections));
            }
            Selection::InlineFragment(inline) => {
                // Recursively extract fields from inline fragments
                fields.extend(extract_fields_from_selections(&inline.selections));
            }
            Selection::FragmentSpread(_) => {
                // Fragment spreads don't contain fields directly
            }
        }
    }

    fields
}

/// Recursively extract all fragment spreads from a selection hierarchy (flattened)
fn extract_fragment_spreads_from_selections(selections: &[Selection]) -> Vec<FragmentSpread> {
    let mut spreads = Vec::new();

    for selection in selections {
        match selection {
            Selection::Field(field_selection) => {
                // Recursively extract fragment spreads from nested fields
                spreads.extend(extract_fragment_spreads_from_selections(
                    &field_selection.selections,
                ));
            }
            Selection::FragmentSpread(spread) => {
                spreads.push(spread.clone());
            }
            Selection::InlineFragment(inline) => {
                // Recursively extract fragment spreads from inline fragments
                spreads.extend(extract_fragment_spreads_from_selections(&inline.selections));
            }
        }
    }

    spreads
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
    use crate::parsers::typescript_parser;
    use std::fs;
    use std::path::PathBuf;

    /// Helper function to format selections recursively with proper indentation
    fn format_selections(result: &mut String, selections: &[Selection], indent_level: usize) {
        let indent = "  ".repeat(indent_level);

        for selection in selections {
            match selection {
                Selection::Field(field) => {
                    result.push_str(&format!("{}- Field: {}", indent, field.name));
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
                            result.push_str(&format!("{:?} {}", directive.directive_type, emoji));
                        }
                        result.push_str("]");
                    }
                    result.push_str("\n");

                    // Recursively format nested selections
                    if !field.selections.is_empty() {
                        format_selections(result, &field.selections, indent_level + 1);
                    }
                }
                Selection::FragmentSpread(spread) => {
                    result.push_str(&format!("{}- FragmentSpread: {}", indent, spread.name));
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
                            result.push_str(&format!("{:?} {}", directive.directive_type, emoji));
                        }
                        result.push_str("]");
                    }
                    result.push_str("\n");
                }
                Selection::InlineFragment(inline) => {
                    result.push_str(&format!("{}- InlineFragment", indent));
                    if let Some(type_condition) = &inline.type_condition {
                        result.push_str(&format!(" on {}", type_condition));
                    }
                    if !inline.directives.is_empty() {
                        result.push_str(" [");
                        for (j, directive) in inline.directives.iter().enumerate() {
                            if j > 0 {
                                result.push_str(", ");
                            }
                            let emoji = match directive.directive_type {
                                DirectiveType::Catch => "🧤",
                                DirectiveType::ThrowOnFieldError => "☄️",
                            };
                            result.push_str(&format!("{:?} {}", directive.directive_type, emoji));
                        }
                        result.push_str("]");
                    }
                    result.push_str("\n");

                    // Recursively format nested selections
                    if !inline.selections.is_empty() {
                        format_selections(result, &inline.selections, indent_level + 1);
                    }
                }
            }
        }
    }

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

                    // Show selections with hierarchical structure
                    result.push_str(&format!("Selections: {}\n", query.selections.len()));
                    format_selections(&mut result, &query.selections, 2);
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

                    // Show type condition for fragments
                    result.push_str(&format!("Type Condition: {}\n", fragment.type_condition));

                    // Show selections with hierarchical structure
                    result.push_str(&format!("Selections: {}\n", fragment.selections.len()));
                    format_selections(&mut result, &fragment.selections, 2);
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
