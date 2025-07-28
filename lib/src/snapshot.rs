use crate::parser::ast_builder::AstBuilder;
use crate::scanner::{extractor::GraphQLExtractor, file_finder::FileFinder};
use crate::tree_formatter::TreeFormatter;
use crate::types::directive::DirectiveType;
use crate::types::graphql::{FragmentDefinition, GraphQLItem, QueryOperation};
use anyhow::Result;
use std::path::Path;

pub struct SnapshotGenerator {
    file_finder: FileFinder,
    extractor: GraphQLExtractor,
    ast_builder: AstBuilder,
}

impl SnapshotGenerator {
    pub fn new() -> Result<Self> {
        Ok(Self {
            file_finder: FileFinder::new("**/*.{ts,tsx}")?,
            extractor: GraphQLExtractor::new(),
            ast_builder: AstBuilder::new(),
        })
    }

    pub fn generate_snapshots_for_directory(&self, dir_path: &Path) -> Result<String> {
        let files = self.file_finder.find_files(dir_path)?;
        let mut snapshots = Vec::new();

        for file_path in files {
            let file_snapshot = self.generate_file_snapshot(&file_path)?;
            if !file_snapshot.is_empty() {
                snapshots.push(format!(
                    "=== {} ===\n{}",
                    file_path.display(),
                    file_snapshot
                ));
            }
        }

        Ok(snapshots.join("\n\n"))
    }

    fn generate_file_snapshot(&self, file_path: &Path) -> Result<String> {
        let graphql_strings = self.extractor.extract_from_file(file_path)?;
        if graphql_strings.is_empty() {
            return Ok(String::new());
        }

        let mut file_tree = TreeFormatter::new();
        file_tree.add_line(0, &format!("File: {}", file_path.display()));

        for (i, graphql_string) in graphql_strings.iter().enumerate() {
            let items = self.ast_builder.build_from_graphql_string(graphql_string)?;

            if !items.is_empty() {
                file_tree.add_line(1, &format!("GraphQL Block {}", i + 1));

                for item in items {
                    let item_tree = self.format_graphql_item(&item);
                    file_tree.add_tree(2, &item_tree);
                }
            }
        }

        Ok(file_tree.to_string())
    }

    fn format_graphql_item(&self, item: &GraphQLItem) -> TreeFormatter {
        match item {
            GraphQLItem::Query(query) => self.format_query(query),
            GraphQLItem::Fragment(fragment) => self.format_fragment(fragment),
        }
    }

    fn format_query(&self, query: &QueryOperation) -> TreeFormatter {
        let mut tree = TreeFormatter::new();

        let directives_str = self.format_directives_inline(&query.directives);
        tree.add_line(0, &format!("Query: {}{}", query.name, directives_str));

        // Add fields
        for field in &query.fields {
            let field_directives = self.format_directives_inline(&field.directives);
            tree.add_line(1, &format!("Field: {}{}", field.name, field_directives));
        }

        // Add fragment spreads
        for fragment in &query.fragments {
            let frag_directives = self.format_directives_inline(&fragment.directives);
            tree.add_line(
                1,
                &format!("Fragment: ...{}{}", fragment.name, frag_directives),
            );
        }

        tree
    }

    fn format_fragment(&self, fragment: &FragmentDefinition) -> TreeFormatter {
        let mut tree = TreeFormatter::new();

        let directives_str = self.format_directives_inline(&fragment.directives);
        tree.add_line(0, &format!("Fragment: {}{}", fragment.name, directives_str));

        // Add fields
        for field in &fragment.fields {
            let field_directives = self.format_directives_inline(&field.directives);
            tree.add_line(1, &format!("Field: {}{}", field.name, field_directives));
        }

        // Add fragment spreads
        for frag_spread in &fragment.fragments {
            let frag_directives = self.format_directives_inline(&frag_spread.directives);
            tree.add_line(
                1,
                &format!("Fragment: ...{}{}", frag_spread.name, frag_directives),
            );
        }

        tree
    }

    fn format_directives_inline(
        &self,
        directives: &[crate::types::directive::Directive],
    ) -> String {
        if directives.is_empty() {
            return String::new();
        }

        let directive_strs: Vec<String> = directives
            .iter()
            .map(|d| match d.directive_type {
                DirectiveType::Catch => "@catch".to_string(),
                DirectiveType::ThrowOnFieldError => "@throwOnFieldError ⚠️".to_string(),
            })
            .collect();

        format!(" {}", directive_strs.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::extractor::GraphQLString;
    use std::path::PathBuf;

    #[test]
    fn test_simple_snapshot() {
        let ast_builder = AstBuilder::new();
        let generator = SnapshotGenerator::new().expect("Failed to create generator");

        // Create a simple GraphQL string to test parsing
        let test_graphql = GraphQLString {
            content: r#"
                query GetUser($id: ID!) @catch {
                    user(id: $id) {
                        id
                        name
                        avatar @throwOnFieldError
                    }
                }
            "#
            .to_string(),
            file_path: PathBuf::from("test.ts"),
            line: 1,
        };

        let items = ast_builder
            .build_from_graphql_string(&test_graphql)
            .expect("Failed to parse GraphQL");

        let mut snapshot_content = String::new();
        for item in items {
            let tree = generator.format_graphql_item(&item);
            snapshot_content.push_str(&tree.to_string());
            snapshot_content.push('\n');
        }

        insta::assert_snapshot!(snapshot_content.trim());
    }

    #[test]
    fn test_valid_fixtures_snapshot() {
        let generator = SnapshotGenerator::new().expect("Failed to create generator");
        let fixtures_dir = PathBuf::from("../fixtures/valid");

        if fixtures_dir.exists() {
            let snapshot = generator
                .generate_snapshots_for_directory(&fixtures_dir)
                .expect("Failed to generate snapshots");

            insta::assert_snapshot!("valid_fixtures", snapshot);

            // Basic assertions
            assert!(snapshot.contains("Query:") || snapshot.contains("Fragment:"));
        } else {
            panic!("Valid fixtures directory not found: {:?}", fixtures_dir);
        }
    }

    #[test]
    fn test_invalid_fixtures_snapshot() {
        let generator = SnapshotGenerator::new().expect("Failed to create generator");
        let fixtures_dir = PathBuf::from("../fixtures/invalid");

        if fixtures_dir.exists() {
            let snapshot = generator
                .generate_snapshots_for_directory(&fixtures_dir)
                .expect("Failed to generate snapshots");

            insta::assert_snapshot!("invalid_fixtures", snapshot);

            // Basic assertions
            assert!(snapshot.contains("Query:") || snapshot.contains("Fragment:"));
        } else {
            panic!("Invalid fixtures directory not found: {:?}", fixtures_dir);
        }
    }

    #[test]
    fn test_edge_cases_fixtures_snapshot() {
        let generator = SnapshotGenerator::new().expect("Failed to create generator");
        let fixtures_dir = PathBuf::from("../fixtures/edge_cases");

        if fixtures_dir.exists() {
            match generator.generate_snapshots_for_directory(&fixtures_dir) {
                Ok(snapshot) => {
                    insta::assert_snapshot!("edge_cases_fixtures", snapshot);
                }
                Err(e) => {
                    // Capture the error message as a snapshot too
                    let error_message = format!("Parsing error: {}", e);
                    insta::assert_snapshot!("edge_cases_error", error_message);
                }
            }
        } else {
            panic!(
                "Edge cases fixtures directory not found: {:?}",
                fixtures_dir
            );
        }
    }
}
