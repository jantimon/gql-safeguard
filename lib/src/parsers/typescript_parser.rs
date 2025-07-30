//! Robust GraphQL extraction using SWC AST parsing
//!
//! Avoids regex false positives from comments, strings, and complex TypeScript syntax.

use anyhow::Result;
use std::fs;
use std::path::Path;
use swc_core::common::BytePos;
use swc_core::ecma::{
    ast::*,
    parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax},
    visit::{Visit, VisitWith},
};

#[derive(Debug, Clone)]
pub struct GraphQLString {
    pub content: String,
    pub file_path: std::path::PathBuf,
    pub position: u32,
}

// Finds GraphQL in TS/TSX files while avoiding dynamic content that can't be validated
pub fn extract_graphql_from_file(file_path: &Path) -> Result<Vec<GraphQLString>> {
    let source_code = fs::read_to_string(file_path)?;

    // Performance optimization: skip AST parsing for files without GraphQL
    if !source_code.contains("gql") && !source_code.contains("graphql") {
        return Ok(Vec::new());
    }

    let syntax = if file_path.extension().and_then(|s| s.to_str()) == Some("tsx") {
        Syntax::Typescript(TsSyntax {
            tsx: true,
            ..Default::default()
        })
    } else {
        Syntax::Typescript(TsSyntax {
            tsx: false,
            ..Default::default()
        })
    };

    let lexer = Lexer::new(
        syntax,
        Default::default(),
        StringInput::new(&source_code, BytePos(0), BytePos(source_code.len() as u32)),
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
    // Initialize visitor state for file processing
    fn new(file_path: std::path::PathBuf) -> Self {
        Self {
            file_path,
            graphql_strings: Vec::new(),
        }
    }

    // Core extraction logic: identifies GraphQL templates and extracts static content
    fn extract_graphql_from_tagged_template(&mut self, tpl: &TaggedTpl) {
        // Check if this is a GraphQL tagged template (gql`...` or graphql`...`)
        if let Expr::Ident(ident) = &*tpl.tag {
            if ident.sym.as_ref() == "gql" || ident.sym.as_ref() == "graphql" {
                // Skip dynamic templates - runtime values could change GraphQL structure
                if tpl.tpl.quasis.len() > 1 {
                    return;
                }

                // Capture static GraphQL string with position info for error reporting
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

// SWC visitor pattern for AST traversal
impl Visit for GraphQLVisitor {
    // Automatically called by SWC for each tagged template in the AST
    fn visit_tagged_tpl(&mut self, tpl: &TaggedTpl) {
        // Check if this template contains GraphQL content
        self.extract_graphql_from_tagged_template(tpl);

        // Ensure complete AST traversal for nested templates
        tpl.visit_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    // Consistent test output format with portable file paths
    fn format_extraction_result(file_path: &Path, graphql_strings: &[GraphQLString]) -> String {
        // Portable paths prevent test failures across different machines
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

    // Process entire fixture directory for comprehensive testing
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

            // Deterministic ordering prevents flaky tests
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

    // Validates extraction from well-formed GraphQL template literals
    #[test]
    fn test_valid_fixtures() {
        let result = test_fixture_directory("valid");
        insta::assert_snapshot!(result);
    }

    // Ensures extraction works even for GraphQL with validation issues
    #[test]
    fn test_invalid_fixtures() {
        let result = test_fixture_directory("invalid");
        insta::assert_snapshot!(result);
    }

    // Tests complex scenarios like comments, interpolation, and nested templates
    #[test]
    fn test_edge_case_fixtures() {
        let result = test_fixture_directory("edge_cases");
        insta::assert_snapshot!(result);
    }
}
