use crate::extraction::graphql_parser::FragmentDefinition;
use rustc_hash::FxHashMap;

pub struct FragmentRegistry {
    fragments: FxHashMap<String, FragmentDefinition>,
}

impl Default for FragmentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FragmentRegistry {
    pub fn new() -> Self {
        Self {
            fragments: FxHashMap::default(),
        }
    }

    pub fn add_fragment(&mut self, fragment: FragmentDefinition) {
        self.fragments.insert(fragment.name.clone(), fragment);
    }

    pub fn get_fragment(&self, name: &str) -> Option<&FragmentDefinition> {
        self.fragments.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &FragmentDefinition)> {
        self.fragments.iter()
    }
}
