//! GraphQL AST parsing with directive extraction for safety validation
//!
//! Converts raw GraphQL strings into structured data focusing on @catch/@throwOnFieldError
//! directives needed for runtime error prevention analysis.

use std::path::PathBuf;

use crate::parsers::typescript_parser::GraphQLString;
use anyhow::Result;
use graphql_parser::parse_query;
use graphql_parser::query::{
    Definition, Document as QueryDocument, OperationDefinition, SelectionSet,
};
use serde::{Deserialize, Serialize};

// Backward compatibility for modules expecting flat field lists
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub directives: Vec<Directive>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectiveType {
    Catch,
    ThrowOnFieldError,
    RequiredThrow,
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

// Entry point: converts GraphQL strings to AST with safety-relevant directives
pub fn parse_graphql_to_ast(graphql_string: &GraphQLString) -> Result<Vec<GraphQLItem>> {
    // Validate GraphQL syntax and build AST representation
    let document: QueryDocument<String> = parse_query(&graphql_string.content).map_err(|e| {
        anyhow::anyhow!(
            "GraphQL syntax error in {} at position {}: {:?}",
            graphql_string.file_path.display(),
            graphql_string.position,
            e
        )
    })?;

    let mut items = Vec::new();

    // Extract queries and fragments with their directive information
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

// Converts parsed operations to internal format for validation
// Focuses on queries since @throwOnFieldError is query-specific
fn convert_operation_to_query(
    op: OperationDefinition<String>,
    file_path: &std::path::Path,
    position: u32,
) -> Result<Option<QueryOperation>> {
    match op {
        OperationDefinition::Query(query) => {
            // Anonymous queries need names for error reporting
            let name = query.name.unwrap_or_else(|| "AnonymousQuery".to_string());

            // Query-level directives affect all nested selections
            let directives = extract_directives_from_directive_list(&query.directives, position);

            // Maintain nesting for proper directive inheritance validation
            let selections = convert_selection_set(&query.selection_set, position);

            Ok(Some(QueryOperation {
                name,
                selections,
                directives,
                file_path: file_path.to_path_buf(),
            }))
        }
        OperationDefinition::Mutation(_) | OperationDefinition::Subscription(_) => {
            // @throwOnFieldError is primarily used in data-fetching queries
            Ok(None)
        }
        OperationDefinition::SelectionSet(_) => {
            // Rare pattern - focus on named operations for now
            Ok(None)
        }
    }
}

// Converts fragments for dependency resolution and validation
// Fragments are key for @catch protection inheritance
fn convert_fragment_definition(
    frag: graphql_parser::query::FragmentDefinition<String>,
    file_path: &std::path::Path,
    position: u32,
) -> Result<FragmentDefinition> {
    // Fragment-level directives protect all contained selections
    let directives = extract_directives_from_directive_list(&frag.directives, position);

    // Maintain structure for nested directive validation
    let selections = convert_selection_set(&frag.selection_set, position);

    Ok(FragmentDefinition {
        name: frag.name,
        type_condition: frag.type_condition.to_string(),
        selections,
        directives,
        file_path: file_path.to_path_buf(),
    })
}

// Builds hierarchical structure preserving directive inheritance relationships
// Critical for validating @catch protection across nested selections
fn convert_selection_set(selection_set: &SelectionSet<String>, position: u32) -> Vec<Selection> {
    let mut selections = Vec::new();

    for selection in &selection_set.items {
        match selection {
            graphql_parser::query::Selection::Field(field) => {
                // Field directives can provide or require protection
                let directives =
                    extract_directives_from_directive_list(&field.directives, position);

                // Fields may contain nested selections needing validation
                let nested_selections = convert_selection_set(&field.selection_set, position);

                selections.push(Selection::Field(FieldSelection {
                    name: field.name.clone(),
                    directives,
                    selections: nested_selections,
                }));
            }
            graphql_parser::query::Selection::FragmentSpread(spread) => {
                // Spread directives can add protection before fragment expansion
                let directives =
                    extract_directives_from_directive_list(&spread.directives, position);

                selections.push(Selection::FragmentSpread(FragmentSpread {
                    name: spread.fragment_name.clone(),
                    directives,
                }));
            }
            graphql_parser::query::Selection::InlineFragment(inline) => {
                // Inline fragments can provide @catch protection
                let directives =
                    extract_directives_from_directive_list(&inline.directives, position);

                // Process inline fragment contents
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

// Backward compatibility: convert hierarchical structure to flat lists
impl QueryOperation {
    // Legacy API: flatten hierarchy to simple field list
    pub fn fields(&self) -> Vec<Field> {
        extract_fields_from_selections(&self.selections)
    }

    // Legacy API: collect all fragment references
    pub fn fragments(&self) -> Vec<FragmentSpread> {
        extract_fragment_spreads_from_selections(&self.selections)
    }
}

impl FragmentDefinition {
    // Legacy API: flatten hierarchy to simple field list
    pub fn fields(&self) -> Vec<Field> {
        extract_fields_from_selections(&self.selections)
    }

    // Legacy API: collect all fragment references
    pub fn fragments(&self) -> Vec<FragmentSpread> {
        extract_fragment_spreads_from_selections(&self.selections)
    }
}

// Depth-first traversal to collect all field selections
fn extract_fields_from_selections(selections: &[Selection]) -> Vec<Field> {
    let mut fields = Vec::new();

    for selection in selections {
        match selection {
            Selection::Field(field_selection) => {
                fields.push(Field {
                    name: field_selection.name.clone(),
                    directives: field_selection.directives.clone(),
                });
                // Collect fields from nested selections
                fields.extend(extract_fields_from_selections(&field_selection.selections));
            }
            Selection::InlineFragment(inline) => {
                // Inline fragments may contain additional fields
                fields.extend(extract_fields_from_selections(&inline.selections));
            }
            Selection::FragmentSpread(_) => {
                // Spreads reference external fragments
            }
        }
    }

    fields
}

// Collect all fragment references for dependency analysis
fn extract_fragment_spreads_from_selections(selections: &[Selection]) -> Vec<FragmentSpread> {
    let mut spreads = Vec::new();

    for selection in selections {
        match selection {
            Selection::Field(field_selection) => {
                // Check nested selections for more fragment spreads
                spreads.extend(extract_fragment_spreads_from_selections(
                    &field_selection.selections,
                ));
            }
            Selection::FragmentSpread(spread) => {
                spreads.push(spread.clone());
            }
            Selection::InlineFragment(inline) => {
                // Inline fragments may reference other fragments
                spreads.extend(extract_fragment_spreads_from_selections(&inline.selections));
            }
        }
    }

    spreads
}

// Filters and converts directives to internal representation
// Only processes @catch, @throwOnFieldError, and @required(action: THROW) - ignores irrelevant directives
fn extract_directives_from_directive_list(
    directives: &[graphql_parser::query::Directive<String>],
    position: u32,
) -> Vec<Directive> {
    directives
        .iter()
        .filter_map(|dir| {
            // Skip directives that don't affect error handling safety
            let directive_type = match dir.name.as_str() {
                "catch" => DirectiveType::Catch,
                "throwOnFieldError" => DirectiveType::ThrowOnFieldError,
                "required" => {
                    // Only process @required if it has action: THROW
                    if has_throw_action(&dir.arguments) {
                        DirectiveType::RequiredThrow
                    } else {
                        return None; // Ignore @required with other actions
                    }
                }
                _ => return None,
            };

            Some(Directive {
                directive_type,
                position,
            })
        })
        .collect()
}

// Helper function to check if @required directive has action: THROW
fn has_throw_action(arguments: &[(String, graphql_parser::query::Value<String>)]) -> bool {
    arguments.iter().any(|(name, value)| {
        name == "action"
            && matches!(value, graphql_parser::query::Value::Enum(action) if action == "THROW")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::typescript_parser;
    use std::fs;
    use std::path::PathBuf;

    // Builds hierarchical visualization of parsed GraphQL structure
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
                                DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                                    "☄️"
                                }
                            };
                            result.push_str(&format!("{:?} {}", directive.directive_type, emoji));
                        }
                        result.push_str("]");
                    }
                    result.push_str("\n");

                    // Show nested structure with increased indentation
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
                                DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                                    "☄️"
                                }
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
                                DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                                    "☄️"
                                }
                            };
                            result.push_str(&format!("{:?} {}", directive.directive_type, emoji));
                        }
                        result.push_str("]");
                    }
                    result.push_str("\n");

                    // Show fragment content structure
                    if !inline.selections.is_empty() {
                        format_selections(result, &inline.selections, indent_level + 1);
                    }
                }
            }
        }
    }

    // Consistent test output showing complete AST structure
    fn format_graphql_ast_result(
        file_path: &std::path::Path,
        graphql_items: &[GraphQLItem],
    ) -> String {
        // Portable paths prevent test differences across machines
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

                    // Ensure all paths are portable in test output
                    let query_file_relative = query
                        .file_path
                        .strip_prefix(&git_root)
                        .unwrap_or(&query.file_path);
                    result.push_str(&format!("File: {}\n", query_file_relative.display()));

                    // Visual indicators help identify directive types
                    result.push_str(&format!("Directives: {}\n", query.directives.len()));
                    for directive in &query.directives {
                        let emoji = match directive.directive_type {
                            DirectiveType::Catch => "🧤",
                            DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => "☄️",
                        };
                        result.push_str(&format!("  - {:?} {}\n", directive.directive_type, emoji));
                    }

                    // Display complete query structure for debugging
                    result.push_str(&format!("Selections: {}\n", query.selections.len()));
                    format_selections(&mut result, &query.selections, 2);
                }
                GraphQLItem::Fragment(fragment) => {
                    result.push_str(&format!("Type: Fragment\n"));
                    result.push_str(&format!("Name: {}\n", fragment.name));

                    // Consistent path formatting across all test output
                    let fragment_file_relative = fragment
                        .file_path
                        .strip_prefix(&git_root)
                        .unwrap_or(&fragment.file_path);
                    result.push_str(&format!("File: {}\n", fragment_file_relative.display()));

                    // Visual markers for quick directive identification
                    result.push_str(&format!("Directives: {}\n", fragment.directives.len()));
                    for directive in &fragment.directives {
                        let emoji = match directive.directive_type {
                            DirectiveType::Catch => "🧤",
                            DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => "☄️",
                        };
                        result.push_str(&format!("  - {:?} {}\n", directive.directive_type, emoji));
                    }

                    // Display fragment target type for context
                    result.push_str(&format!("Type Condition: {}\n", fragment.type_condition));

                    // Complete fragment structure for analysis
                    result.push_str(&format!("Selections: {}\n", fragment.selections.len()));
                    format_selections(&mut result, &fragment.selections, 2);
                }
            }
            result.push_str("\n");
        }

        result
    }

    // Comprehensive testing across all fixture files with detailed AST output
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

            // Deterministic ordering prevents test flakiness
            files.sort_by_key(|entry| entry.file_name());

            for entry in files {
                let file_path = entry.path();

                // Two-stage parsing: TS extraction then GraphQL parsing
                match typescript_parser::extract_graphql_from_file(&file_path) {
                    Ok(graphql_strings) => {
                        // Convert extracted strings to structured AST
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

    // Validates AST generation for well-formed GraphQL
    #[test]
    fn test_parse_valid_fixtures_to_ast() {
        let result = process_fixture_directory_to_ast("valid");
        insta::assert_snapshot!(result);
    }

    // Ensures parser handles problematic GraphQL gracefully
    #[test]
    fn test_parse_invalid_fixtures_to_ast() {
        let result = process_fixture_directory_to_ast("invalid");
        insta::assert_snapshot!(result);
    }

    // Complex scenarios like nested fragments and unusual patterns
    #[test]
    fn test_parse_edge_case_fixtures_to_ast() {
        let result = process_fixture_directory_to_ast("edge_cases");
        insta::assert_snapshot!(result);
    }
}
