use anyhow::Result;
use std::fs;
use std::path::Path;
use swc_core::ecma::{
    ast::*,
    parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig},
    visit::{Visit, VisitWith},
};

pub struct GraphQLExtractor;

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
