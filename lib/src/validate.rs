//! GraphQL Directive Validation
//!
//! This module validates that GraphQL queries follow proper error handling patterns:
//! - Every `@catch` directive must protect at least one `@throwOnFieldError` in its subtree
//! - Every `@throwOnFieldError` directive must be protected by at least one `@catch` ancestor
//!
//! The validation uses a single-pass recursive traversal with O(n) time complexity,
//! tracking ancestor `@catch` directives to ensure proper protection relationships.

use anyhow::Result;
use rustc_hash::FxHashSet;
use std::fmt;

use crate::parsers::graphql_parser::{DirectiveType, Selection};
use crate::registry_to_graph::QueryWithFragments;

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

/// A validation error with context about the query and location
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub query_name: String,
    pub error_type: ValidationErrorType,
    pub location: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} in query '{}' at {}",
            self.error_type, self.error_type, self.query_name, self.location
        )
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
/// * `Ok(())` if all queries are valid
/// * `Err(ValidationError)` with details about the first validation failure found
///
/// # Performance
/// * Time complexity: O(n) where n is the total number of selections across all queries
/// * Space complexity: O(d) where d is the maximum depth of nested selections
pub fn validate_query_directives(queries: &[QueryWithFragments]) -> Result<()> {
    for query in queries {
        validate_query(query)?;
    }
    Ok(())
}

/// Validates a single query for proper directive protection patterns
fn validate_query(query: &QueryWithFragments) -> Result<()> {
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
        &query.name,
        "query",
        &mut catch_ancestors,
        &mut protecting_catches,
    )?;

    // Check for empty @catch directives at query level
    for directive in &query.directives {
        if directive.directive_type == DirectiveType::Catch
            && !protecting_catches.contains(&directive.position)
        {
            return Err(ValidationError {
                query_name: query.name.clone(),
                error_type: ValidationErrorType::EmptyCatch,
                location: "query level".to_string(),
            }
            .into());
        }
    }

    Ok(())
}

/// Recursively validates selections with ancestor tracking
fn validate_selections(
    selections: &[Selection],
    query_name: &str,
    current_location: &str,
    catch_ancestors: &mut FxHashSet<u32>,
    protecting_catches: &mut FxHashSet<u32>,
) -> Result<()> {
    for selection in selections {
        match selection {
            Selection::Field(field) => {
                let field_location = format!("{}.{}", current_location, field.name);

                // Check field-level directives
                validate_field_directives(
                    field,
                    query_name,
                    &field_location,
                    catch_ancestors,
                    protecting_catches,
                )?;

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
                    query_name,
                    &field_location,
                    &mut field_catch_ancestors,
                    protecting_catches,
                )?;

                // Check for empty @catch directives at field level
                for directive in &field.directives {
                    if directive.directive_type == DirectiveType::Catch
                        && !protecting_catches.contains(&directive.position)
                    {
                        return Err(ValidationError {
                            query_name: query_name.to_string(),
                            error_type: ValidationErrorType::EmptyCatch,
                            location: field_location.clone(),
                        }
                        .into());
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
                                return Err(ValidationError {
                                    query_name: query_name.to_string(),
                                    error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                                    location: spread_location.clone(),
                                }
                                .into());
                            }
                            // Mark all ancestor @catch directives as protecting
                            protecting_catches.extend(catch_ancestors.iter());
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

                // Check inline fragment directives
                for directive in &inline.directives {
                    match directive.directive_type {
                        DirectiveType::ThrowOnFieldError => {
                            if catch_ancestors.is_empty() {
                                return Err(ValidationError {
                                    query_name: query_name.to_string(),
                                    error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                                    location: inline_location.clone(),
                                }
                                .into());
                            }
                            // Mark all ancestor @catch directives as protecting
                            protecting_catches.extend(catch_ancestors.iter());
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
                    query_name,
                    &inline_location,
                    &mut fragment_catch_ancestors,
                    protecting_catches,
                )?;

                // Check for empty @catch directives at fragment level
                for directive in &inline.directives {
                    if directive.directive_type == DirectiveType::Catch
                        && !protecting_catches.contains(&directive.position)
                    {
                        return Err(ValidationError {
                            query_name: query_name.to_string(),
                            error_type: ValidationErrorType::EmptyCatch,
                            location: inline_location.clone(),
                        }
                        .into());
                    }
                }
            }
        }
    }

    Ok(())
}

/// Validates directives on a specific field
fn validate_field_directives(
    field: &crate::parsers::graphql_parser::FieldSelection,
    query_name: &str,
    field_location: &str,
    catch_ancestors: &FxHashSet<u32>,
    protecting_catches: &mut FxHashSet<u32>,
) -> Result<()> {
    for directive in &field.directives {
        match directive.directive_type {
            DirectiveType::ThrowOnFieldError => {
                if catch_ancestors.is_empty() {
                    return Err(ValidationError {
                        query_name: query_name.to_string(),
                        error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                        location: field_location.to_string(),
                    }
                    .into());
                }
                // Mark all ancestor @catch directives as protecting
                protecting_catches.extend(catch_ancestors.iter());
            }
            DirectiveType::Catch => {
                // @catch directive validation happens after processing nested selections
            }
        }
    }
    Ok(())
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
            result.is_ok(),
            "Valid fixtures should pass validation: {:?}",
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
        assert!(result.is_err(), "Invalid fixtures should fail validation");

        // Snapshot the validation error for regression testing
        let error_message = format!("Validation Error: {}", result.unwrap_err());
        insta::assert_snapshot!(error_message);
    }

    #[test]
    fn test_validate_edge_cases() {
        let files = collect_fixture_files("edge_cases");
        let registry = process_files(&files);

        // Edge cases might include circular dependencies, so handle build errors
        match registry_to_dependency_graph(&registry) {
            Ok(dependency_graph) => {
                let result = validate_query_directives(&dependency_graph);
                // Snapshot the result (could be success or validation error)
                let result_message = match result {
                    Ok(_) => "All edge cases passed validation".to_string(),
                    Err(e) => format!("Edge case validation error: {}", e),
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
