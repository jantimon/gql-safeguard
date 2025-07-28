//! GraphQL Directive Validation
//!
//! This module validates that GraphQL queries follow proper error handling patterns:
//! - Every `@catch` directive must protect at least one `@throwOnFieldError` in its subtree
//! - Every `@throwOnFieldError` directive must be protected by at least one `@catch` ancestor
//!
//! The validation uses a single-pass recursive traversal with O(n) time complexity,
//! tracking ancestor `@catch` directives to ensure proper protection relationships.

use rustc_hash::FxHashSet;
use std::fmt;
use std::path::PathBuf;

use crate::parsers::graphql_parser::{DirectiveType, Selection};
use crate::registry_to_graph::QueryWithFragments;
use crate::tree_formatter::TreeFormatter;

/// Validation error types for directive protection violations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationErrorType {
    /// A `@throwOnFieldError` directive without any protecting `@catch` ancestor
    UnprotectedThrowOnFieldError,
    /// A `@catch` directive that doesn't protect any `@throwOnFieldError` in its subtree
    EmptyCatch,
}

impl fmt::Display for ValidationErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationErrorType::UnprotectedThrowOnFieldError => {
                write!(f, "Unprotected @throwOnFieldError")
            }
            ValidationErrorType::EmptyCatch => {
                write!(f, "Empty @catch (no @throwOnFieldError in subtree)")
            }
        }
    }
}

/// Context about where an error occurred in the query structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    pub query_name: String,
    pub query_file: PathBuf,
    pub location_path: String,
    pub fragment_file: Option<PathBuf>, // Set if error is in a fragment
    pub fragment_name: Option<String>,  // Set if error is in a fragment
}

/// A validation error with rich context and tree visualization
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub error_type: ValidationErrorType,
    pub context: ErrorContext,
    pub tree_visualization: String,
    pub explanation: String,
}

/// Collection of all validation errors found during validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationResult {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n🚨 {}", self.error_type)?;
        writeln!(f)?;

        // Show query and file information
        writeln!(
            f,
            "Query: {} ({})",
            self.context.query_name,
            self.context.query_file.display()
        )?;
        if let (Some(fragment_name), Some(fragment_file)) =
            (&self.context.fragment_name, &self.context.fragment_file)
        {
            writeln!(
                f,
                "Fragment: {} ({})",
                fragment_name,
                fragment_file.display()
            )?;
        }
        // Show simplified location (fragment.field format)
        let simplified_location = if self.context.location_path.starts_with("query.") {
            self.context
                .location_path
                .strip_prefix("query.")
                .unwrap_or(&self.context.location_path)
        } else {
            &self.context.location_path
        };

        // Simplify to FRAGMENT_NAME.FIELD_NAME format only
        let final_location = if let Some(last_dot_pos) = simplified_location.rfind('.') {
            let field_name = &simplified_location[last_dot_pos + 1..];
            // Look for the last fragment name (after the last "...")
            let before_field = &simplified_location[..last_dot_pos];
            if let Some(last_fragment_pos) = before_field.rfind("...") {
                let fragment_name = &before_field[last_fragment_pos + 3..];
                format!("{}.{}", fragment_name, field_name)
            } else {
                // No fragment found, just use the field name
                field_name.to_string()
            }
        } else {
            simplified_location.to_string()
        };

        writeln!(f, "Location: {}", final_location)?;
        writeln!(f)?;

        // Show tree visualization
        writeln!(f, "Query Structure:")?;
        write!(f, "{}", self.tree_visualization)
    }
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.errors.is_empty() {
            write!(
                f,
                "✅ All queries follow proper @catch protection patterns!"
            )
        } else {
            // Summary
            writeln!(
                f,
                "❌ Found {} validation error{}:",
                self.errors.len(),
                if self.errors.len() == 1 { "" } else { "s" }
            )?;
            writeln!(f)?;

            // Individual errors
            for (i, error) in self.errors.iter().enumerate() {
                if i > 0 {
                    writeln!(f)?;
                    writeln!(f, "{}", "-".repeat(80))?;
                }
                writeln!(f)?;
                write!(f, "{}", error)?;
            }
            Ok(())
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validates that all GraphQL queries follow proper directive protection patterns
///
/// This function ensures that:
/// 1. Every `@catch` directive protects at least one `@throwOnFieldError` in its subtree
/// 2. Every `@throwOnFieldError` directive has at least one `@catch` ancestor
///
/// # Arguments
/// * `queries` - The queries with resolved fragment dependencies to validate
///
/// # Returns
/// * `ValidationResult` containing all validation errors found, with rich context and explanations
///
/// # Performance
/// * Time complexity: O(n) where n is the total number of selections across all queries
/// * Space complexity: O(d) where d is the maximum depth of nested selections
pub fn validate_query_directives(queries: &[QueryWithFragments]) -> ValidationResult {
    let mut result = ValidationResult::new();

    for query in queries {
        validate_query(query, &mut result);
    }

    result
}

/// Validates a single query for proper directive protection patterns
fn validate_query(query: &QueryWithFragments, result: &mut ValidationResult) {
    // Track which @catch directives in this query protect at least one @throwOnFieldError
    let mut protecting_catches = FxHashSet::default();

    // Track ancestor @catch directives (positions) during traversal
    let mut catch_ancestors = FxHashSet::default();

    // Add query-level @catch directives to ancestors
    for directive in &query.directives {
        if directive.directive_type == DirectiveType::Catch {
            catch_ancestors.insert(directive.position);
        }
    }

    // Validate all selections in the query
    validate_selections(
        &query.selections,
        query,
        "query",
        &mut catch_ancestors,
        &mut protecting_catches,
        result,
        None, // No fragment context at query level
        None,
    );

    // Check for empty @catch directives at query level
    for directive in &query.directives {
        if directive.directive_type == DirectiveType::Catch
            && !protecting_catches.contains(&directive.position)
        {
            let context = ErrorContext {
                query_name: query.name.clone(),
                query_file: query.file_path.clone(),
                location_path: "query level".to_string(),
                fragment_file: None,
                fragment_name: None,
            };

            let tree_visualization = create_query_tree_visualization(query, Some("query level"));
            let explanation = create_empty_catch_explanation();

            result.add_error(ValidationError {
                error_type: ValidationErrorType::EmptyCatch,
                context,
                tree_visualization,
                explanation,
            });
        }
    }
}

/// Recursively validates selections with ancestor tracking
fn validate_selections(
    selections: &[Selection],
    query: &QueryWithFragments,
    current_location: &str,
    catch_ancestors: &mut FxHashSet<u32>,
    protecting_catches: &mut FxHashSet<u32>,
    result: &mut ValidationResult,
    current_fragment_file: Option<PathBuf>,
    current_fragment_name: Option<String>,
) {
    for selection in selections {
        match selection {
            Selection::Field(field) => {
                let field_location = format!("{}.{}", current_location, field.name);

                // Check field-level directives
                validate_field_directives(
                    field,
                    query,
                    &field_location,
                    catch_ancestors,
                    protecting_catches,
                    result,
                    current_fragment_file.clone(),
                    current_fragment_name.clone(),
                );

                // Create new ancestor context for this field's nested selections
                let mut field_catch_ancestors = catch_ancestors.clone();

                // Add field-level @catch directives to ancestors for nested selections
                for directive in &field.directives {
                    if directive.directive_type == DirectiveType::Catch {
                        field_catch_ancestors.insert(directive.position);
                    }
                }

                // Recursively validate nested selections
                validate_selections(
                    &field.selections,
                    query,
                    &field_location,
                    &mut field_catch_ancestors,
                    protecting_catches,
                    result,
                    current_fragment_file.clone(),
                    current_fragment_name.clone(),
                );

                // Check for empty @catch directives at field level
                for directive in &field.directives {
                    if directive.directive_type == DirectiveType::Catch
                        && !protecting_catches.contains(&directive.position)
                    {
                        let context = ErrorContext {
                            query_name: query.name.clone(),
                            query_file: query.file_path.clone(),
                            location_path: field_location.clone(),
                            fragment_file: current_fragment_file.clone(),
                            fragment_name: current_fragment_name.clone(),
                        };

                        let tree_visualization =
                            create_query_tree_visualization(query, Some(&field_location));
                        let explanation = create_empty_catch_explanation();

                        result.add_error(ValidationError {
                            error_type: ValidationErrorType::EmptyCatch,
                            context,
                            tree_visualization,
                            explanation,
                        });
                    }
                }
            }
            Selection::FragmentSpread(spread) => {
                let spread_location = format!("{}...{}", current_location, spread.name);

                // Check spread-level directives
                for directive in &spread.directives {
                    match directive.directive_type {
                        DirectiveType::ThrowOnFieldError => {
                            if catch_ancestors.is_empty() {
                                let context = ErrorContext {
                                    query_name: query.name.clone(),
                                    query_file: query.file_path.clone(),
                                    location_path: spread_location.clone(),
                                    fragment_file: current_fragment_file.clone(),
                                    fragment_name: Some(spread.name.clone()),
                                };

                                let tree_visualization =
                                    create_query_tree_visualization(query, Some(&spread_location));
                                let explanation = create_unprotected_throw_explanation();

                                result.add_error(ValidationError {
                                    error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                                    context,
                                    tree_visualization,
                                    explanation,
                                });
                            } else {
                                // Mark all ancestor @catch directives as protecting
                                protecting_catches.extend(catch_ancestors.iter());
                            }
                        }
                        DirectiveType::Catch => {
                            // Fragment spread @catch will be validated when fragment content is processed
                        }
                    }
                }
            }
            Selection::InlineFragment(inline) => {
                let fragment_name = inline
                    .type_condition
                    .as_ref()
                    .and_then(|tc| tc.strip_suffix("Fragment"))
                    .unwrap_or("InlineFragment");
                let inline_location = format!("{}...{}", current_location, fragment_name);

                // Determine if this is a resolved fragment (has type_condition ending with "Fragment")
                let is_resolved_fragment = inline
                    .type_condition
                    .as_ref()
                    .map(|tc| tc.ends_with("Fragment"))
                    .unwrap_or(false);

                let fragment_context_file = if is_resolved_fragment {
                    // TODO: We would need to track fragment file paths through the resolution process
                    // For now, we'll use the current fragment file or None
                    current_fragment_file.clone()
                } else {
                    current_fragment_file.clone()
                };

                let fragment_context_name = if is_resolved_fragment {
                    Some(fragment_name.to_string())
                } else {
                    current_fragment_name.clone()
                };

                // Check inline fragment directives
                for directive in &inline.directives {
                    match directive.directive_type {
                        DirectiveType::ThrowOnFieldError => {
                            if catch_ancestors.is_empty() {
                                let context = ErrorContext {
                                    query_name: query.name.clone(),
                                    query_file: query.file_path.clone(),
                                    location_path: inline_location.clone(),
                                    fragment_file: fragment_context_file.clone(),
                                    fragment_name: fragment_context_name.clone(),
                                };

                                let tree_visualization =
                                    create_query_tree_visualization(query, Some(&inline_location));
                                let explanation = create_unprotected_throw_explanation();

                                result.add_error(ValidationError {
                                    error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                                    context,
                                    tree_visualization,
                                    explanation,
                                });
                            } else {
                                // Mark all ancestor @catch directives as protecting
                                protecting_catches.extend(catch_ancestors.iter());
                            }
                        }
                        DirectiveType::Catch => {
                            // Will be added to ancestors below
                        }
                    }
                }

                // Create new ancestor context for this fragment's nested selections
                let mut fragment_catch_ancestors = catch_ancestors.clone();

                // Add fragment-level @catch directives to ancestors for nested selections
                for directive in &inline.directives {
                    if directive.directive_type == DirectiveType::Catch {
                        fragment_catch_ancestors.insert(directive.position);
                    }
                }

                // Recursively validate fragment selections
                validate_selections(
                    &inline.selections,
                    query,
                    &inline_location,
                    &mut fragment_catch_ancestors,
                    protecting_catches,
                    result,
                    fragment_context_file.clone(),
                    fragment_context_name.clone(),
                );

                // Check for empty @catch directives at fragment level
                for directive in &inline.directives {
                    if directive.directive_type == DirectiveType::Catch
                        && !protecting_catches.contains(&directive.position)
                    {
                        let context = ErrorContext {
                            query_name: query.name.clone(),
                            query_file: query.file_path.clone(),
                            location_path: inline_location.clone(),
                            fragment_file: fragment_context_file.clone(),
                            fragment_name: fragment_context_name.clone(),
                        };

                        let tree_visualization =
                            create_query_tree_visualization(query, Some(&inline_location));
                        let explanation = create_empty_catch_explanation();

                        result.add_error(ValidationError {
                            error_type: ValidationErrorType::EmptyCatch,
                            context,
                            tree_visualization,
                            explanation,
                        });
                    }
                }
            }
        }
    }
}

/// Validates directives on a specific field
fn validate_field_directives(
    field: &crate::parsers::graphql_parser::FieldSelection,
    query: &QueryWithFragments,
    field_location: &str,
    catch_ancestors: &FxHashSet<u32>,
    protecting_catches: &mut FxHashSet<u32>,
    result: &mut ValidationResult,
    current_fragment_file: Option<PathBuf>,
    current_fragment_name: Option<String>,
) {
    for directive in &field.directives {
        match directive.directive_type {
            DirectiveType::ThrowOnFieldError => {
                if catch_ancestors.is_empty() {
                    let context = ErrorContext {
                        query_name: query.name.clone(),
                        query_file: query.file_path.clone(),
                        location_path: field_location.to_string(),
                        fragment_file: current_fragment_file.clone(),
                        fragment_name: current_fragment_name.clone(),
                    };

                    let tree_visualization =
                        create_query_tree_visualization(query, Some(field_location));
                    let explanation = create_unprotected_throw_explanation();

                    result.add_error(ValidationError {
                        error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                        context,
                        tree_visualization,
                        explanation,
                    });
                } else {
                    // Mark all ancestor @catch directives as protecting
                    protecting_catches.extend(catch_ancestors.iter());
                }
            }
            DirectiveType::Catch => {
                // @catch directive validation happens after processing nested selections
            }
        }
    }
}

/// Creates a tree visualization of the query structure, highlighting the error location
fn create_query_tree_visualization(
    query: &QueryWithFragments,
    error_location: Option<&str>,
) -> String {
    let mut formatter = TreeFormatter::new();

    // Git root for relative paths
    let git_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let relative_path = query
        .file_path
        .strip_prefix(&git_root)
        .unwrap_or(&query.file_path);

    formatter.add_line(
        0,
        &format!("📄 Query: {} ({})", query.name, relative_path.display()),
    );

    // Add query-level directives
    if !query.directives.is_empty() {
        formatter.add_line(1, "🏷️  Query Directives:");
        for directive in &query.directives {
            let emoji = match directive.directive_type {
                DirectiveType::Catch => "🧤",
                DirectiveType::ThrowOnFieldError => "⚠️",
            };
            let highlight = if let Some(error_loc) = error_location {
                if error_loc == "query level" {
                    " ❌"
                } else {
                    ""
                }
            } else {
                ""
            };
            formatter.add_line(
                2,
                &format!("{} @{:?}{}", emoji, directive.directive_type, highlight),
            );
        }
    }

    // Add query selections
    if !query.selections.is_empty() {
        formatter.add_line(1, "🔍 Selections:");
        format_selections_for_error(&mut formatter, &query.selections, 2, error_location);
    }

    formatter.to_string()
}

/// Recursively formats selections for error visualization, highlighting the error location
fn format_selections_for_error(
    formatter: &mut TreeFormatter,
    selections: &[Selection],
    depth: usize,
    error_location: Option<&str>,
) {
    for selection in selections {
        match selection {
            Selection::Field(field) => {
                let highlight = if let Some(error_loc) = error_location {
                    if error_loc.ends_with(&format!(".{}", field.name)) {
                        " ❌"
                    } else {
                        ""
                    }
                } else {
                    ""
                };

                let mut field_text = format!("🔹 Field: {}{}", field.name, highlight);

                if !field.directives.is_empty() {
                    let directive_strs: Vec<String> = field
                        .directives
                        .iter()
                        .map(|d| {
                            let emoji = match d.directive_type {
                                DirectiveType::Catch => "🧤",
                                DirectiveType::ThrowOnFieldError => "⚠️",
                            };
                            format!("{} @{:?}", emoji, d.directive_type)
                        })
                        .collect();
                    field_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                }

                formatter.add_line(depth, &field_text);

                if !field.selections.is_empty() {
                    format_selections_for_error(
                        formatter,
                        &field.selections,
                        depth + 1,
                        error_location,
                    );
                }
            }
            Selection::FragmentSpread(spread) => {
                let highlight = if let Some(error_loc) = error_location {
                    if error_loc.contains(&format!("...{}", spread.name)) {
                        " ❌"
                    } else {
                        ""
                    }
                } else {
                    ""
                };

                let mut spread_text = format!("📋 FragmentSpread: {}{}", spread.name, highlight);

                if !spread.directives.is_empty() {
                    let directive_strs: Vec<String> = spread
                        .directives
                        .iter()
                        .map(|d| {
                            let emoji = match d.directive_type {
                                DirectiveType::Catch => "🧤",
                                DirectiveType::ThrowOnFieldError => "⚠️",
                            };
                            format!("{} @{:?}", emoji, d.directive_type)
                        })
                        .collect();
                    spread_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                }

                formatter.add_line(depth, &spread_text);
            }
            Selection::InlineFragment(inline) => {
                let fragment_name = inline
                    .type_condition
                    .as_ref()
                    .and_then(|tc| tc.strip_suffix("Fragment"))
                    .unwrap_or("InlineFragment");

                let highlight = if let Some(error_loc) = error_location {
                    if error_loc.contains(&format!("...{}", fragment_name)) {
                        " ❌"
                    } else {
                        ""
                    }
                } else {
                    ""
                };

                let mut inline_text = format!("🧩 Fragment: {}{}", fragment_name, highlight);

                if !inline.directives.is_empty() {
                    let directive_strs: Vec<String> = inline
                        .directives
                        .iter()
                        .map(|d| {
                            let emoji = match d.directive_type {
                                DirectiveType::Catch => "🧤",
                                DirectiveType::ThrowOnFieldError => "⚠️",
                            };
                            format!("{} @{:?}", emoji, d.directive_type)
                        })
                        .collect();
                    inline_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                }

                formatter.add_line(depth, &inline_text);

                if !inline.selections.is_empty() {
                    format_selections_for_error(
                        formatter,
                        &inline.selections,
                        depth + 1,
                        error_location,
                    );
                }
            }
        }
    }
}

/// Creates an explanation for unprotected @throwOnFieldError errors
fn create_unprotected_throw_explanation() -> String {
    String::new() // No individual explanations needed since we have global explanation
}

/// Creates an explanation for empty @catch errors  
fn create_empty_catch_explanation() -> String {
    String::new() // No individual explanations needed since we have global explanation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::process_files;
    use crate::registry_to_graph::registry_to_dependency_graph;
    use std::fs;
    use std::path::PathBuf;

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

    #[test]
    fn test_validate_valid_fixtures() {
        let files = collect_fixture_files("valid");
        let registry = process_files(&files);
        let dependency_graph =
            registry_to_dependency_graph(&registry).expect("Failed to build dependency graph");

        let result = validate_query_directives(&dependency_graph);
        assert!(
            result.is_valid(),
            "Valid fixtures should pass validation but found {} errors: {}",
            result.errors.len(),
            result
        );
    }

    #[test]
    fn test_validate_invalid_fixtures() {
        let files = collect_fixture_files("invalid");
        let registry = process_files(&files);
        let dependency_graph =
            registry_to_dependency_graph(&registry).expect("Failed to build dependency graph");

        let result = validate_query_directives(&dependency_graph);
        assert!(
            result.has_errors(),
            "Invalid fixtures should fail validation"
        );

        // Snapshot the validation result for regression testing
        let result_message = format!("Validation Result:\n{}", result);
        insta::assert_snapshot!(result_message);
    }

    #[test]
    fn test_validate_edge_cases() {
        let files = collect_fixture_files("edge_cases");
        let registry = process_files(&files);

        // Edge cases might include circular dependencies, so handle build errors
        match registry_to_dependency_graph(&registry) {
            Ok(dependency_graph) => {
                let result = validate_query_directives(&dependency_graph);
                // Snapshot the result (could be success or validation errors)
                let result_message = if result.is_valid() {
                    "All edge cases passed validation".to_string()
                } else {
                    format!("Edge case validation result:\n{}", result)
                };
                insta::assert_snapshot!(result_message);
            }
            Err(e) => {
                // Graph building failed (e.g., circular dependencies)
                let error_message = format!("Graph building failed for edge cases: {}", e);
                insta::assert_snapshot!(error_message);
            }
        }
    }
}
