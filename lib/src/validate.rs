//! Prevents GraphQL runtime errors by enforcing @catch protection
//!
//! Validates @throwOnFieldError directives have proper @catch protection to prevent
//! uncaught exceptions from breaking entire pages in Relay applications.

use rustc_hash::FxHashSet;
use std::fmt;
use std::path::PathBuf;

use crate::parsers::graphql_parser::{DirectiveType, Selection};
use crate::registry_to_graph::QueryWithFragments;
use crate::tree_formatter::TreeFormatter;

struct ValidationContext<'a> {
    query: &'a QueryWithFragments,
    catch_ancestors: &'a mut FxHashSet<u32>,
    protecting_catches: &'a mut FxHashSet<u32>,
    result: &'a mut ValidationResult,
    current_fragment_file: Option<PathBuf>,
    current_fragment_name: Option<String>,
}

// Different safety violations require different remediation strategies
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationErrorType {
    // Dangerous: field errors will propagate as exceptions
    UnprotectedThrowOnFieldError,
    // Unnecessary: no throwing fields to protect from
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

// Rich location info helps developers quickly find and fix issues
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    pub query_name: String,
    pub query_file: PathBuf,
    pub location_path: String,
    pub fragment_file: Option<PathBuf>, // Set if error is in a fragment
    pub fragment_name: Option<String>,  // Set if error is in a fragment
}

// Combines error details with visual query structure for easy debugging
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub error_type: ValidationErrorType,
    pub context: ErrorContext,
    pub tree_visualization: String,
    pub explanation: String,
}

// Aggregates all issues to show complete safety picture
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

        // Git root for relative paths in snapshots
        let git_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();

        // Show query and file information with relative paths
        let query_relative_path = self
            .context
            .query_file
            .strip_prefix(&git_root)
            .unwrap_or(&self.context.query_file);
        writeln!(
            f,
            "Query: {} ({})",
            self.context.query_name,
            query_relative_path.display()
        )?;
        if let (Some(fragment_name), Some(fragment_file)) =
            (&self.context.fragment_name, &self.context.fragment_file)
        {
            let fragment_relative_path = fragment_file
                .strip_prefix(&git_root)
                .unwrap_or(fragment_file);
            writeln!(
                f,
                "Fragment: {} ({})",
                fragment_name,
                fragment_relative_path.display()
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
                format!("{fragment_name}.{field_name}")
            } else {
                // No fragment found, just use the field name
                field_name.to_string()
            }
        } else {
            simplified_location.to_string()
        };

        writeln!(f, "Location: {final_location}")?;
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
                write!(f, "{error}")?;
            }
            Ok(())
        }
    }
}

impl std::error::Error for ValidationError {}

// Entry point for comprehensive GraphQL safety validation
// Prevents runtime exceptions by ensuring proper @catch/@throw protection patterns
pub fn validate_query_directives(queries: &[QueryWithFragments]) -> ValidationResult {
    let mut result = ValidationResult::new();

    for query in queries {
        validate_query(query, &mut result);
    }

    result
}

// Per-query validation with ancestor tracking for protection chains
fn validate_query(query: &QueryWithFragments, result: &mut ValidationResult) {
    // Identifies useful @catch directives (vs unused ones)
    let mut protecting_catches = FxHashSet::default();

    // Maintains protection context as we traverse query tree
    let mut catch_ancestors = FxHashSet::default();

    // Query-level @catch protects all nested selections
    for directive in &query.directives {
        if directive.directive_type == DirectiveType::Catch {
            catch_ancestors.insert(directive.position);
        }
    }

    // Recursive validation with protection context
    let mut ctx = ValidationContext {
        query,
        catch_ancestors: &mut catch_ancestors,
        protecting_catches: &mut protecting_catches,
        result,
        current_fragment_file: None, // No fragment context at query level
        current_fragment_name: None,
    };
    validate_selections(&query.selections, "query", &mut ctx);

    // Report unused @catch that protect nothing
    for directive in &query.directives {
        if directive.directive_type == DirectiveType::Catch
            && !ctx.protecting_catches.contains(&directive.position)
        {
            let context = ErrorContext {
                query_name: ctx.query.name.clone(),
                query_file: ctx.query.file_path.clone(),
                location_path: "query level".to_string(),
                fragment_file: None,
                fragment_name: None,
            };

            let tree_visualization =
                create_query_tree_visualization(ctx.query, Some("query level"));
            let explanation = create_empty_catch_explanation();

            ctx.result.add_error(ValidationError {
                error_type: ValidationErrorType::EmptyCatch,
                context,
                tree_visualization,
                explanation,
            });
        }
    }
}

// Core validation logic - maintains @catch ancestry during tree traversal
fn validate_selections(
    selections: &[Selection],
    current_location: &str,
    ctx: &mut ValidationContext,
) {
    for selection in selections {
        match selection {
            Selection::Field(field) => {
                let field_location = format!("{}.{}", current_location, field.name);

                // Check field-level directives
                validate_field_directives(field, &field_location, ctx);

                // Create new ancestor context for this field's nested selections
                let mut field_catch_ancestors = ctx.catch_ancestors.clone();

                // Add field-level @catch directives to ancestors for nested selections
                for directive in &field.directives {
                    if directive.directive_type == DirectiveType::Catch {
                        field_catch_ancestors.insert(directive.position);
                    }
                }

                // Recursively validate nested selections
                let mut nested_ctx = ValidationContext {
                    query: ctx.query,
                    catch_ancestors: &mut field_catch_ancestors,
                    protecting_catches: ctx.protecting_catches,
                    result: ctx.result,
                    current_fragment_file: ctx.current_fragment_file.clone(),
                    current_fragment_name: ctx.current_fragment_name.clone(),
                };
                validate_selections(&field.selections, &field_location, &mut nested_ctx);

                // Check for empty @catch directives at field level
                for directive in &field.directives {
                    if directive.directive_type == DirectiveType::Catch
                        && !ctx.protecting_catches.contains(&directive.position)
                    {
                        let context = ErrorContext {
                            query_name: ctx.query.name.clone(),
                            query_file: ctx.query.file_path.clone(),
                            location_path: field_location.clone(),
                            fragment_file: ctx.current_fragment_file.clone(),
                            fragment_name: ctx.current_fragment_name.clone(),
                        };

                        let tree_visualization =
                            create_query_tree_visualization(ctx.query, Some(&field_location));
                        let explanation = create_empty_catch_explanation();

                        ctx.result.add_error(ValidationError {
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
                        DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                            if ctx.catch_ancestors.is_empty() {
                                let context = ErrorContext {
                                    query_name: ctx.query.name.clone(),
                                    query_file: ctx.query.file_path.clone(),
                                    location_path: spread_location.clone(),
                                    fragment_file: ctx.current_fragment_file.clone(),
                                    fragment_name: Some(spread.name.clone()),
                                };

                                let tree_visualization = create_query_tree_visualization(
                                    ctx.query,
                                    Some(&spread_location),
                                );
                                let explanation = create_unprotected_throw_explanation();

                                ctx.result.add_error(ValidationError {
                                    error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                                    context,
                                    tree_visualization,
                                    explanation,
                                });
                            } else {
                                // Mark all ancestor @catch directives as protecting
                                ctx.protecting_catches.extend(ctx.catch_ancestors.iter());
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
                let inline_location = format!("{current_location}...{fragment_name}");

                // Determine if this is a resolved fragment (has type_condition ending with "Fragment")
                let is_resolved_fragment = inline
                    .type_condition
                    .as_ref()
                    .map(|tc| tc.ends_with("Fragment"))
                    .unwrap_or(false);

                let fragment_context_file = if is_resolved_fragment {
                    // TODO: We would need to track fragment file paths through the resolution process
                    // For now, we'll use the current fragment file or None
                    ctx.current_fragment_file.clone()
                } else {
                    ctx.current_fragment_file.clone()
                };

                let fragment_context_name = if is_resolved_fragment {
                    Some(fragment_name.to_string())
                } else {
                    ctx.current_fragment_name.clone()
                };

                // Check inline fragment directives
                for directive in &inline.directives {
                    match directive.directive_type {
                        DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                            if ctx.catch_ancestors.is_empty() {
                                let context = ErrorContext {
                                    query_name: ctx.query.name.clone(),
                                    query_file: ctx.query.file_path.clone(),
                                    location_path: inline_location.clone(),
                                    fragment_file: fragment_context_file.clone(),
                                    fragment_name: fragment_context_name.clone(),
                                };

                                let tree_visualization = create_query_tree_visualization(
                                    ctx.query,
                                    Some(&inline_location),
                                );
                                let explanation = create_unprotected_throw_explanation();

                                ctx.result.add_error(ValidationError {
                                    error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                                    context,
                                    tree_visualization,
                                    explanation,
                                });
                            } else {
                                // Mark all ancestor @catch directives as protecting
                                ctx.protecting_catches.extend(ctx.catch_ancestors.iter());
                            }
                        }
                        DirectiveType::Catch => {
                            // Will be added to ancestors below
                        }
                    }
                }

                // Create new ancestor context for this fragment's nested selections
                let mut fragment_catch_ancestors = ctx.catch_ancestors.clone();

                // Add fragment-level @catch directives to ancestors for nested selections
                for directive in &inline.directives {
                    if directive.directive_type == DirectiveType::Catch {
                        fragment_catch_ancestors.insert(directive.position);
                    }
                }

                // Recursively validate fragment selections
                let mut fragment_ctx = ValidationContext {
                    query: ctx.query,
                    catch_ancestors: &mut fragment_catch_ancestors,
                    protecting_catches: ctx.protecting_catches,
                    result: ctx.result,
                    current_fragment_file: fragment_context_file.clone(),
                    current_fragment_name: fragment_context_name.clone(),
                };
                validate_selections(&inline.selections, &inline_location, &mut fragment_ctx);

                // Check for empty @catch directives at fragment level
                for directive in &inline.directives {
                    if directive.directive_type == DirectiveType::Catch
                        && !ctx.protecting_catches.contains(&directive.position)
                    {
                        let context = ErrorContext {
                            query_name: ctx.query.name.clone(),
                            query_file: ctx.query.file_path.clone(),
                            location_path: inline_location.clone(),
                            fragment_file: fragment_context_file.clone(),
                            fragment_name: fragment_context_name.clone(),
                        };

                        let tree_visualization =
                            create_query_tree_visualization(ctx.query, Some(&inline_location));
                        let explanation = create_empty_catch_explanation();

                        ctx.result.add_error(ValidationError {
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

// Field-level validation with protection context from ancestors
fn validate_field_directives(
    field: &crate::parsers::graphql_parser::FieldSelection,
    field_location: &str,
    ctx: &mut ValidationContext,
) {
    for directive in &field.directives {
        match directive.directive_type {
            DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                if ctx.catch_ancestors.is_empty() {
                    let context = ErrorContext {
                        query_name: ctx.query.name.clone(),
                        query_file: ctx.query.file_path.clone(),
                        location_path: field_location.to_string(),
                        fragment_file: ctx.current_fragment_file.clone(),
                        fragment_name: ctx.current_fragment_name.clone(),
                    };

                    let tree_visualization =
                        create_query_tree_visualization(ctx.query, Some(field_location));
                    let explanation = create_unprotected_throw_explanation();

                    ctx.result.add_error(ValidationError {
                        error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                        context,
                        tree_visualization,
                        explanation,
                    });
                } else {
                    // Mark all ancestor @catch directives as protecting
                    ctx.protecting_catches.extend(ctx.catch_ancestors.iter());
                }
            }
            DirectiveType::Catch => {
                // @catch directive validation happens after processing nested selections
            }
        }
    }
}

// Visual debugging aid to quickly locate problems in complex queries
fn create_query_tree_visualization(
    query: &QueryWithFragments,
    error_location: Option<&str>,
) -> String {
    let mut formatter = TreeFormatter::new();

    // Git root for relative paths in snapshots
    let git_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();

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
                DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => "☄️",
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

// Builds visual tree showing query structure with error markers
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
                                DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                                    "☄️"
                                }
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
                                DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                                    "☄️"
                                }
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
                    if error_loc.contains(&format!("...{fragment_name}")) {
                        " ❌"
                    } else {
                        ""
                    }
                } else {
                    ""
                };

                let mut inline_text = format!("🧩 Fragment: {fragment_name}{highlight}");

                if !inline.directives.is_empty() {
                    let directive_strs: Vec<String> = inline
                        .directives
                        .iter()
                        .map(|d| {
                            let emoji = match d.directive_type {
                                DirectiveType::Catch => "🧤",
                                DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                                    "☄️"
                                }
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

// Guidance text helping developers understand protection requirements
fn create_unprotected_throw_explanation() -> String {
    String::new() // No individual explanations needed since we have global explanation
}

// Explains why unused @catch directives should be removed
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

    // Deterministic test file ordering for consistent snapshots
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
