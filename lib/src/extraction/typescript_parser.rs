//! TypeScript/TSX Parser for GraphQL Template Literals
//!
//! Uses SWC AST parsing instead of regex to avoid false positives from commented GraphQL.

use anyhow::Result;
use std::fs;
use std::path::Path;
use swc_core::ecma::{
    ast::*,
    parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig},
    visit::{Visit, VisitWith},
};

#[derive(Debug, Clone)]
pub struct GraphQLString {
    pub content: String,
    pub file_path: std::path::PathBuf,
    pub position: u32,
}

/// Extracts GraphQL from `gql` and `graphql` tagged template literals.
/// Skips templates with interpolation since they can't be statically analyzed.
pub fn extract_graphql_from_file(file_path: &Path) -> Result<Vec<GraphQLString>> {
    let source_code = fs::read_to_string(file_path)?;

    let syntax = if file_path.extension().and_then(|s| s.to_str()) == Some("tsx") {
        Syntax::Typescript(TsConfig {
            tsx: true,
            ..Default::default()
        })
    } else {
        Syntax::Typescript(TsConfig {
            tsx: false,
            ..Default::default()
        })
    };

    let lexer = Lexer::new(
        syntax,
        Default::default(),
        StringInput::new(&source_code, Default::default(), Default::default()),
        None,
    );

    let mut parser = Parser::new_from(lexer);
    let module = parser.parse_module().map_err(|e| {
        anyhow::anyhow!("TypeScript parse error in {}: {:?}", file_path.display(), e)
    })?;

    let mut visitor = GraphQLVisitor::new(file_path.to_path_buf());
    module.visit_with(&mut visitor);

    Ok(visitor.graphql_strings)
}

struct GraphQLVisitor {
    file_path: std::path::PathBuf,
    graphql_strings: Vec<GraphQLString>,
}

impl GraphQLVisitor {
    /// Creates a new visitor for the specified file path.
    ///
    /// # Arguments
    /// * `file_path` - Path to the file being processed
    fn new(file_path: std::path::PathBuf) -> Self {
        Self {
            file_path,
            graphql_strings: Vec::new(),
        }
    }

    /// Extracts GraphQL content from a tagged template literal if it matches our criteria.
    ///
    /// This method checks if the tagged template uses `gql` or `graphql` as the tag,
    /// and if so, extracts the template content.
    ///
    /// Skips templates with interpolation since those cannot be statically analyzed
    ///
    fn extract_graphql_from_tagged_template(&mut self, tpl: &TaggedTpl) {
        // Check if this is a GraphQL tagged template (gql`...` or graphql`...`)
        if let Expr::Ident(ident) = &*tpl.tag {
            if ident.sym.as_ref() == "gql" || ident.sym.as_ref() == "graphql" {
                // Skip template literals with interpolation (more than one quasi)
                // These cannot be statically analyzed since the interpolated values
                // are determined at runtime and could affect GraphQL structure
                if tpl.tpl.quasis.len() > 1 {
                    return;
                }

                // Extract the GraphQL content from the template literal
                if let Some(first_quasi) = tpl.tpl.quasis.first() {
                    let content = first_quasi.raw.as_ref();
                    self.graphql_strings.push(GraphQLString {
                        content: content.to_string(),
                        file_path: self.file_path.clone(),
                        position: first_quasi.span.lo().0,
                    });
                }
            }
        }
    }
}

/// Implementation of SWC's Visit trait for GraphQLVisitor.
impl Visit for GraphQLVisitor {
    /// Visits tagged template literal nodes in the AST.
    ///
    /// This method is called automatically by SWC's visitor framework whenever
    /// a tagged template literal is encountered during AST traversal. It processes
    /// the node for GraphQL extraction and then continues traversing child nodes.
    fn visit_tagged_tpl(&mut self, tpl: &TaggedTpl) {
        // Process this tagged template for GraphQL extraction
        self.extract_graphql_from_tagged_template(tpl);

        // Continue visiting child nodes in the AST
        tpl.visit_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Formats extraction results for snapshot testing with relative file paths.
    ///
    /// This helper function converts extraction results into a consistent string format
    /// suitable for snapshot testing. It uses relative paths from the git root to
    /// ensure snapshots are portable across different development environments.
    ///
    /// # Arguments
    /// * `file_path` - Absolute path to the processed file
    /// * `graphql_strings` - List of extracted GraphQL strings
    ///
    /// # Returns
    /// Formatted string showing file path, count, and each GraphQL string with metadata
    fn format_extraction_result(file_path: &Path, graphql_strings: &[GraphQLString]) -> String {
        // Convert to relative path from git root for portable snapshots
        let git_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let relative_path = file_path.strip_prefix(&git_root).unwrap_or(file_path);

        let mut result = format!("File: {}\n", relative_path.display());
        result.push_str(&format!(
            "GraphQL strings found: {}\n\n",
            graphql_strings.len()
        ));

        for (i, gql_string) in graphql_strings.iter().enumerate() {
            result.push_str(&format!("=== GraphQL String {} ===\n", i + 1));
            result.push_str(&format!("BytePos: {}\n", gql_string.position));
            result.push_str("Content:\n");
            result.push_str(&gql_string.content);
            result.push_str("\n\n");
        }

        result
    }

    /// Tests extraction from all files in a fixture directory.
    ///
    /// This helper function processes all TypeScript/TSX files in the specified
    /// fixture directory and formats the results for snapshot comparison. It ensures
    /// consistent ordering by sorting files by name.
    ///
    /// # Arguments
    /// * `dir_name` - Name of the fixture directory (valid, invalid, edge_cases)
    ///
    /// # Returns
    /// Formatted string containing extraction results from all files in the directory
    fn test_fixture_directory(dir_name: &str) -> String {
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
                match extract_graphql_from_file(&file_path) {
                    Ok(graphql_strings) => {
                        let result = format_extraction_result(&file_path, &graphql_strings);
                        results.push(result);
                    }
                    Err(e) => {
                        results.push(format!("File: {}\nError: {}\n\n", file_path.display(), e));
                    }
                }
            }
        }

        results.join("---\n\n")
    }

    /// Tests GraphQL extraction from all valid fixture files.
    ///
    /// This test processes all files in the `fixtures/valid/` directory and creates
    /// a snapshot of the extraction results. Valid fixtures should contain properly
    /// structured GraphQL with appropriate `@catch` and `@throwOnFieldError` directives.
    #[test]
    fn test_valid_fixtures() {
        let result = test_fixture_directory("valid");
        insta::assert_snapshot!(result);
    }

    /// Tests GraphQL extraction from all invalid fixture files.
    ///
    /// This test processes all files in the `fixtures/invalid/` directory and creates
    /// a snapshot of the extraction results. Invalid fixtures contain GraphQL that
    /// violates the `@catch` protection rules.
    #[test]
    fn test_invalid_fixtures() {
        let result = test_fixture_directory("invalid");
        insta::assert_snapshot!(result);
    }

    /// Tests GraphQL extraction from all edge case fixture files.
    ///
    /// This test processes all files in the `fixtures/edge_cases/` directory and creates
    /// a snapshot of the extraction results. Edge cases include scenarios like commented
    /// GraphQL, circular fragment dependencies, and complex nesting patterns.
    #[test]
    fn test_edge_case_fixtures() {
        let result = test_fixture_directory("edge_cases");
        insta::assert_snapshot!(result);
    }
}
