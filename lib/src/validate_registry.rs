//! Optimized GraphQL validation with smart subtree skipping
//!
//! Validates @throwOnFieldError directives have proper @catch protection by working
//! directly with the registry instead of expanding all fragments. Provides significant
//! performance improvements through subtree skipping and parallel processing.

use rayon::prelude::*;
use rustc_hash::FxHashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::parsers::graphql_parser::{DirectiveType, Selection};
use crate::registry::GraphQLRegistry;
use crate::tree_formatter::TreeFormatter;

// Validation error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationErrorType {
    UnprotectedThrowOnFieldError,
}

impl std::fmt::Display for ValidationErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationErrorType::UnprotectedThrowOnFieldError => {
                write!(f, "Unprotected @throwOnFieldError")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    pub query_name: String,
    pub query_file: PathBuf,
    pub location_path: String,
    pub fragment_file: Option<PathBuf>,
    pub fragment_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub error_type: ValidationErrorType,
    pub context: ErrorContext,
    pub tree_visualization: String,
    pub explanation: String,
}

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

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

// Protection state for efficient subtree skipping
#[derive(Debug, Clone)]
enum ProtectionState {
    Protected,   // @catch found, skip all nested validation
    Unprotected, // No @catch protection, must validate all directives
}

// Validation context with optimized protection tracking
struct RegistryValidationContext<'a> {
    registry: &'a GraphQLRegistry,
    query_name: &'a str,
    query_file: &'a std::path::Path,
    protection_state: ProtectionState,
    current_fragment_file: Option<PathBuf>,
    current_fragment_name: Option<String>,
    errors: Vec<ValidationError>,
    visiting_fragments: FxHashSet<String>, // Cycle detection
}

// Entry point for optimized registry-based validation
// Provides significant performance improvements over dependency graph approach
pub fn validate_registry(registry: &GraphQLRegistry) -> ValidationResult {
    // Thread-safe error collection for parallel processing
    let errors_mutex = Mutex::new(Vec::new());

    // Collect queries into vector for parallel processing
    let queries: Vec<_> = registry
        .queries
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect();

    // Process queries in parallel for maximum performance
    queries.par_iter().for_each(|(query_name, query)| {
        // Initialize per-query validation context
        let mut ctx = RegistryValidationContext {
            registry,
            query_name,
            query_file: &query.file_path,
            protection_state: ProtectionState::Unprotected,
            current_fragment_file: None,
            current_fragment_name: None,
            errors: Vec::new(),
            visiting_fragments: FxHashSet::default(),
        };

        // Check for query-level @catch protection
        let has_query_catch = query
            .directives
            .iter()
            .any(|d| d.directive_type == DirectiveType::Catch);

        if has_query_catch {
            ctx.protection_state = ProtectionState::Protected;
        }

        // Validate query-level directives
        for directive in &query.directives {
            match directive.directive_type {
                DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                    if let ProtectionState::Unprotected = ctx.protection_state {
                        let context = ErrorContext {
                            query_name: ctx.query_name.to_string(),
                            query_file: ctx.query_file.to_path_buf(),
                            location_path: "query level".to_string(),
                            fragment_file: None,
                            fragment_name: None,
                        };

                        let tree_visualization = create_optimized_tree_visualization(
                            ctx.registry,
                            ctx.query_name,
                            ctx.query_file,
                            Some("query level"),
                        );
                        let explanation = String::new();

                        ctx.errors.push(ValidationError {
                            error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                            context,
                            tree_visualization,
                            explanation,
                        });
                    }
                }
                _ => {} // Other directives (like @catch) don't need validation
            }
        }

        // Validate query selections with smart subtree skipping
        validate_selections_optimized(&query.selections, "query", &mut ctx);

        // Collect errors in thread-safe manner
        if !ctx.errors.is_empty() {
            let mut global_errors = errors_mutex.lock().unwrap();
            global_errors.extend(ctx.errors);
        }
    });

    // Aggregate all errors from parallel processing
    let mut all_errors = errors_mutex.into_inner().unwrap();

    // Sort errors for deterministic output (same as sequential processing)
    all_errors.sort_by(|a, b| {
        // First sort by query name, then by location
        a.context
            .query_name
            .cmp(&b.context.query_name)
            .then_with(|| a.context.location_path.cmp(&b.context.location_path))
    });

    ValidationResult { errors: all_errors }
}

// Core optimized validation logic with protection state tracking
fn validate_selections_optimized(
    selections: &[Selection],
    current_location: &str,
    ctx: &mut RegistryValidationContext,
) {
    for selection in selections {
        match selection {
            Selection::Field(field) => {
                let field_location = format!("{}.{}", current_location, field.name);

                // Check field-level @catch for protection
                let field_has_catch = field
                    .directives
                    .iter()
                    .any(|d| d.directive_type == DirectiveType::Catch);

                // Validate field directives based on PARENT's protection state
                validate_field_directives_optimized(field, &field_location, ctx);

                // Create new protection state for this field's CHILDREN
                let original_state = ctx.protection_state.clone();
                if field_has_catch {
                    ctx.protection_state = ProtectionState::Protected;
                }

                // Recursively validate nested selections with updated protection state
                validate_selections_optimized(&field.selections, &field_location, ctx);

                // Restore original protection state
                ctx.protection_state = original_state;
            }
            Selection::FragmentSpread(spread) => {
                let spread_location = format!("{}...{}", current_location, spread.name);

                // OPTIMIZATION: Skip fragment processing if we're in a protected subtree
                match ctx.protection_state {
                    ProtectionState::Protected => {
                        // Skip entire fragment - we know it's protected
                        continue;
                    }
                    ProtectionState::Unprotected => {
                        // Must validate fragment content on-demand
                        validate_fragment_spread_optimized(spread, &spread_location, ctx);
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

                // Check inline fragment @catch protection
                let fragment_has_catch = inline
                    .directives
                    .iter()
                    .any(|d| d.directive_type == DirectiveType::Catch);

                let original_state = ctx.protection_state.clone();
                if fragment_has_catch {
                    ctx.protection_state = ProtectionState::Protected;
                }

                // Validate inline fragment directives
                validate_inline_fragment_directives_optimized(inline, &inline_location, ctx);

                // Recursively validate fragment selections
                validate_selections_optimized(&inline.selections, &inline_location, ctx);

                // Restore original protection state
                ctx.protection_state = original_state;
            }
        }
    }
}

// Optimized field directive validation with protection awareness
fn validate_field_directives_optimized(
    field: &crate::parsers::graphql_parser::FieldSelection,
    field_location: &str,
    ctx: &mut RegistryValidationContext,
) {
    for directive in &field.directives {
        match directive.directive_type {
            DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                // OPTIMIZATION: Only validate if not protected
                if let ProtectionState::Unprotected = ctx.protection_state {
                    let context = ErrorContext {
                        query_name: ctx.query_name.to_string(),
                        query_file: ctx.query_file.to_path_buf(),
                        location_path: field_location.to_string(),
                        fragment_file: ctx.current_fragment_file.clone(),
                        fragment_name: ctx.current_fragment_name.clone(),
                    };

                    let tree_visualization = create_optimized_tree_visualization(
                        ctx.registry,
                        ctx.query_name,
                        ctx.query_file,
                        Some(field_location),
                    );
                    let explanation = String::new();

                    ctx.errors.push(ValidationError {
                        error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                        context,
                        tree_visualization,
                        explanation,
                    });
                }
            }
            DirectiveType::Catch => {
                // @catch directive validation happens in selection processing
            }
        }
    }
}

// On-demand fragment resolution and validation with cycle detection
fn validate_fragment_spread_optimized(
    spread: &crate::parsers::graphql_parser::FragmentSpread,
    spread_location: &str,
    ctx: &mut RegistryValidationContext,
) {
    // Check spread-level directives first
    for directive in &spread.directives {
        match directive.directive_type {
            DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                if let ProtectionState::Unprotected = ctx.protection_state {
                    let context = ErrorContext {
                        query_name: ctx.query_name.to_string(),
                        query_file: ctx.query_file.to_path_buf(),
                        location_path: spread_location.to_string(),
                        fragment_file: ctx.current_fragment_file.clone(),
                        fragment_name: Some(spread.name.clone()),
                    };

                    let tree_visualization = create_optimized_tree_visualization(
                        ctx.registry,
                        ctx.query_name,
                        ctx.query_file,
                        Some(spread_location),
                    );
                    let explanation = String::new();

                    ctx.errors.push(ValidationError {
                        error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                        context,
                        tree_visualization,
                        explanation,
                    });
                }
            }
            DirectiveType::Catch => {
                // Spread has @catch, so entire fragment becomes protected
                ctx.protection_state = ProtectionState::Protected;
                return; // Skip processing fragment content
            }
        }
    }

    // Prevent infinite recursion from circular fragments
    if ctx.visiting_fragments.contains(&spread.name) {
        // Circular dependency detected - skip to prevent stack overflow
        return;
    }

    // Resolve fragment on-demand only if needed
    if let Some(fragment_entry) = ctx.registry.fragments.get(&spread.name) {
        let fragment = fragment_entry.value();

        // Check if fragment itself has @catch protection
        let fragment_has_catch = fragment
            .directives
            .iter()
            .any(|d| d.directive_type == DirectiveType::Catch);

        let original_state = ctx.protection_state.clone();
        let original_fragment_file = ctx.current_fragment_file.clone();
        let original_fragment_name = ctx.current_fragment_name.clone();

        if fragment_has_catch {
            ctx.protection_state = ProtectionState::Protected;
        }

        // Update fragment context
        ctx.current_fragment_file = Some(fragment.file_path.clone());
        ctx.current_fragment_name = Some(fragment.name.clone());

        // Add to visiting set for cycle detection
        ctx.visiting_fragments.insert(spread.name.clone());

        // Validate fragment selections
        validate_selections_optimized(&fragment.selections, spread_location, ctx);

        // Remove from visiting set after processing
        ctx.visiting_fragments.remove(&spread.name);

        // Restore original context
        ctx.protection_state = original_state;
        ctx.current_fragment_file = original_fragment_file;
        ctx.current_fragment_name = original_fragment_name;
    }
}

// Validate inline fragment directives with protection awareness
fn validate_inline_fragment_directives_optimized(
    inline: &crate::parsers::graphql_parser::InlineFragment,
    inline_location: &str,
    ctx: &mut RegistryValidationContext,
) {
    for directive in &inline.directives {
        match directive.directive_type {
            DirectiveType::ThrowOnFieldError | DirectiveType::RequiredThrow => {
                if let ProtectionState::Unprotected = ctx.protection_state {
                    let fragment_name = inline
                        .type_condition
                        .as_ref()
                        .and_then(|tc| tc.strip_suffix("Fragment"))
                        .map(|name| name.to_string());

                    let context = ErrorContext {
                        query_name: ctx.query_name.to_string(),
                        query_file: ctx.query_file.to_path_buf(),
                        location_path: inline_location.to_string(),
                        fragment_file: ctx.current_fragment_file.clone(),
                        fragment_name,
                    };

                    let tree_visualization = create_optimized_tree_visualization(
                        ctx.registry,
                        ctx.query_name,
                        ctx.query_file,
                        Some(inline_location),
                    );
                    let explanation = String::new();

                    ctx.errors.push(ValidationError {
                        error_type: ValidationErrorType::UnprotectedThrowOnFieldError,
                        context,
                        tree_visualization,
                        explanation,
                    });
                }
            }
            DirectiveType::Catch => {
                // Will be handled in selection processing
            }
        }
    }
}

// Create optimized tree visualization without full dependency graph expansion
fn create_optimized_tree_visualization(
    registry: &GraphQLRegistry,
    query_name: &str,
    query_file: &std::path::Path,
    error_location: Option<&str>,
) -> String {
    let mut formatter = TreeFormatter::new();

    // Git root for relative paths
    let git_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();

    let relative_path = query_file.strip_prefix(&git_root).unwrap_or(query_file);

    formatter.add_line(
        0,
        &format!("📄 Query: {} ({})", query_name, relative_path.display()),
    );

    // Get query from registry
    if let Some(query_entry) = registry.queries.get(query_name) {
        let query = query_entry.value();

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
                    &format!("{} @{}{}", emoji, directive.directive_type, highlight),
                );
            }
        }

        // Add query selections
        if !query.selections.is_empty() {
            formatter.add_line(1, "🔍 Selections:");
            format_selections_for_optimized_visualization_with_path(
                &mut formatter,
                &query.selections,
                2,
                error_location,
                registry,
                "query", // Track the current path
            );
        }
    }

    formatter.to_string()
}

// Enhanced selection formatting for optimized visualization with path tracking
fn format_selections_for_optimized_visualization_with_path(
    formatter: &mut TreeFormatter,
    selections: &[Selection],
    depth: usize,
    error_location: Option<&str>,
    registry: &GraphQLRegistry,
    current_path: &str,
) {
    for selection in selections {
        match selection {
            Selection::Field(field) => {
                let field_path = format!("{}.{}", current_path, field.name);
                let highlight = if let Some(error_loc) = error_location {
                    // Precise matching based on full path
                    if error_loc == field_path {
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
                            format!("{} @{}", emoji, d.directive_type)
                        })
                        .collect();
                    field_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                }

                formatter.add_line(depth, &field_text);

                if !field.selections.is_empty() {
                    format_selections_for_optimized_visualization_with_path(
                        formatter,
                        &field.selections,
                        depth + 1,
                        error_location,
                        registry,
                        &field_path,
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
                            format!("{} @{}", emoji, d.directive_type)
                        })
                        .collect();
                    spread_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                }

                formatter.add_line(depth, &spread_text);

                // Optionally show fragment content for better debugging
                if let Some(fragment_entry) = registry.fragments.get(&spread.name) {
                    let fragment = fragment_entry.value();
                    if !fragment.selections.is_empty() {
                        formatter.add_line(depth + 1, "Fragment Content:");
                        format_selections_for_optimized_visualization_with_path(
                            formatter,
                            &fragment.selections,
                            depth + 2,
                            error_location,
                            registry,
                            current_path, // Continue with current path for fragment content
                        );
                    }
                }
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
                            format!("{} @{}", emoji, d.directive_type)
                        })
                        .collect();
                    inline_text.push_str(&format!(" [{}]", directive_strs.join(", ")));
                }

                formatter.add_line(depth, &inline_text);

                if !inline.selections.is_empty() {
                    format_selections_for_optimized_visualization_with_path(
                        formatter,
                        &inline.selections,
                        depth + 1,
                        error_location,
                        registry,
                        current_path, // Continue with current path for inline fragment
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::process_files;
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
    fn test_validate_registry_valid_fixtures() {
        let files = collect_fixture_files("valid");
        let registry = process_files(&files);

        let result = validate_registry(&registry);
        assert!(
            result.is_valid(),
            "Valid fixtures should pass validation but found {} errors: {}",
            result.errors.len(),
            result
        );
    }

    #[test]
    fn test_validate_registry_invalid_fixtures() {
        let files = collect_fixture_files("invalid");
        let registry = process_files(&files);

        let result = validate_registry(&registry);
        assert!(
            result.has_errors(),
            "Invalid fixtures should fail validation"
        );

        // Snapshot the validation result for regression testing
        let result_message = format!("Validation Result:\n{}", result);
        insta::assert_snapshot!(result_message);
    }

    #[test]
    fn test_validate_registry_edge_cases() {
        let files = collect_fixture_files("edge_cases");
        let registry = process_files(&files);

        let result = validate_registry(&registry);
        // Edge cases contain validation errors
        let result_message = if result.is_valid() {
            "All edge cases passed validation".to_string()
        } else {
            format!("Edge case validation result:\n{}", result)
        };
        insta::assert_snapshot!(result_message);
    }
}
