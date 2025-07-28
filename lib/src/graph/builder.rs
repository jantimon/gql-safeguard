use crate::types::directive::Directive;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentNode {
    pub name: String,
    pub children: Vec<FragmentNode>,
    pub directives: Vec<Directive>,
}

pub struct DependencyGraph {
    query_dependencies: FxHashMap<String, Vec<FragmentNode>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            query_dependencies: FxHashMap::default(),
        }
    }

    pub fn add_query_dependencies(&mut self, query_name: String, dependencies: Vec<FragmentNode>) {
        self.query_dependencies.insert(query_name, dependencies);
    }

    pub fn get_dependencies(&self, query_name: &str) -> Option<&Vec<FragmentNode>> {
        self.query_dependencies.get(query_name)
    }
}
