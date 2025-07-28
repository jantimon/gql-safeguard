use anyhow::Result;
use std::fs;
use std::path::Path;
use swc_core::ecma::{
    ast::*,
    parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig},
    visit::{Visit, VisitWith},
};

pub struct GraphQLExtractor;

impl Default for GraphQLExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphQLExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_from_file(&self, file_path: &Path) -> Result<Vec<GraphQLString>> {
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
        let module = parser
            .parse_module()
            .map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;

        let mut visitor = GraphQLVisitor::new(file_path.to_path_buf());
        module.visit_with(&mut visitor);

        Ok(visitor.graphql_strings)
    }
}

#[derive(Debug, Clone)]
pub struct GraphQLString {
    pub content: String,
    pub file_path: std::path::PathBuf,
    pub line: usize,
}

struct GraphQLVisitor {
    file_path: std::path::PathBuf,
    graphql_strings: Vec<GraphQLString>,
}

impl GraphQLVisitor {
    fn new(file_path: std::path::PathBuf) -> Self {
        Self {
            file_path,
            graphql_strings: Vec::new(),
        }
    }

    fn extract_graphql_from_tagged_template(&mut self, tpl: &TaggedTpl) {
        if let Expr::Ident(ident) = &*tpl.tag {
            if ident.sym.as_ref() == "gql" || ident.sym.as_ref() == "graphql" {
                // Skip template literals with interpolation (more than one quasi)
                // These cannot be statically analyzed since the interpolated values
                // are determined at runtime
                if tpl.tpl.quasis.len() > 1 {
                    return;
                }

                if let Some(first_quasi) = tpl.tpl.quasis.first() {
                    let content = first_quasi.raw.as_ref();
                    // Simple line counting - could be improved
                    let line = 1; // Placeholder - would need proper source map

                    self.graphql_strings.push(GraphQLString {
                        content: content.to_string(),
                        file_path: self.file_path.clone(),
                        line,
                    });
                }
            }
        }
    }
}

impl Visit for GraphQLVisitor {
    fn visit_tagged_tpl(&mut self, tpl: &TaggedTpl) {
        self.extract_graphql_from_tagged_template(tpl);
        tpl.visit_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn format_extraction_result(file_path: &Path, graphql_strings: &[GraphQLString]) -> String {
        // Convert to relative path from git root
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
            result.push_str(&format!("Line: {}\n", gql_string.line));
            result.push_str("Content:\n");
            result.push_str(&gql_string.content);
            result.push_str("\n\n");
        }

        result
    }

    fn test_fixture_directory(dir_name: &str) -> String {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("fixtures")
            .join(dir_name);

        let mut results = Vec::new();
        let extractor = GraphQLExtractor::new();

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

            files.sort_by_key(|entry| entry.file_name());

            for entry in files {
                let file_path = entry.path();
                match extractor.extract_from_file(&file_path) {
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

    #[test]
    fn test_valid_fixtures() {
        let result = test_fixture_directory("valid");
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_invalid_fixtures() {
        let result = test_fixture_directory("invalid");
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_edge_case_fixtures() {
        let result = test_fixture_directory("edge_cases");
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_single_file_extraction() {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("fixtures")
            .join("valid")
            .join("fragment_level_catch.ts");

        let extractor = GraphQLExtractor::new();
        let result = extractor.extract_from_file(&fixture_path).unwrap();

        let formatted = format_extraction_result(&fixture_path, &result);
        insta::assert_snapshot!(formatted);
    }
}
