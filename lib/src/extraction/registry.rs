//! GraphQL Extraction Registry
//!

/// A dashmap registry with:
/// process_files()
///
/// a dashmap regisry for all fragments (key fragment name)
/// a dashmap registry for all queries (key query name)
use dashmap::DashMap;
use std::path::Path;
use std::sync::Arc;

use crate::extraction::graphql_parser::{
    parse_graphql_to_ast, FragmentDefinition, GraphQLItem, QueryOperation,
};
use crate::extraction::typescript_parser::extract_graphql_from_file;

/// Registry for GraphQL fragments
pub type FragmentRegistry = Arc<DashMap<String, FragmentDefinition>>;

/// Registry for GraphQL queries
pub type QueryRegistry = Arc<DashMap<String, QueryOperation>>;

/// Main registry that holds both fragments and queries
pub struct GraphQLRegistry {
    pub fragments: FragmentRegistry,
    pub queries: QueryRegistry,
}

impl Default for GraphQLRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphQLRegistry {
    pub fn new() -> Self {
        Self {
            fragments: Arc::new(DashMap::new()),
            queries: Arc::new(DashMap::new()),
        }
    }
}

/// Function which returns a new GraphQL registry for the given file list
pub fn process_files(files: &[String]) -> GraphQLRegistry {
    let registry = GraphQLRegistry::new();

    for file in files {
        parse_file(Path::new(file), &registry);
    }

    registry
}

fn parse_file(file: &Path, registry: &GraphQLRegistry) {
    let graphql_strings_result = extract_graphql_from_file(Path::new(file));
    if let Ok(graphql_strings) = graphql_strings_result {
        for graphql_string in &graphql_strings {
            let graphql_ast = parse_graphql_to_ast(graphql_string);
            if let Ok(ast) = graphql_ast {
                for graphql_item in ast {
                    match graphql_item {
                        GraphQLItem::Fragment(fragment) => {
                            registry.fragments.insert(fragment.name.clone(), fragment);
                        }
                        GraphQLItem::Query(query) => {
                            registry.queries.insert(query.name.clone(), query);
                        }
                    }
                }
            }
        }
    } else {
        eprintln!("Failed to parse GraphQL from file: {}", file.display());
    }
}
