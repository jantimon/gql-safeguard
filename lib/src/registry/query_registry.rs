use crate::extraction::graphql_parser::QueryOperation;
use rustc_hash::FxHashMap;

pub struct QueryRegistry {
    queries: FxHashMap<String, QueryOperation>,
}

impl Default for QueryRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryRegistry {
    pub fn new() -> Self {
        Self {
            queries: FxHashMap::default(),
        }
    }

    pub fn add_query(&mut self, query: QueryOperation) {
        self.queries.insert(query.name.clone(), query);
    }

    pub fn get_query(&self, name: &str) -> Option<&QueryOperation> {
        self.queries.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &QueryOperation)> {
        self.queries.iter()
    }
}
