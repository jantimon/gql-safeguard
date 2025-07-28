use crate::graph::builder::DependencyGraph;
use crate::registry::{fragment_registry::FragmentRegistry, query_registry::QueryRegistry};
use crate::types::violation::Violation;
use anyhow::Result;

pub struct DirectiveValidator;

impl DirectiveValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(
        &self,
        query_registry: &QueryRegistry,
        fragment_registry: &FragmentRegistry,
        dependency_graph: &DependencyGraph,
    ) -> Result<Vec<Violation>> {
        // Placeholder implementation
        // TODO: Validate that @throwOnFieldError has corresponding @catch protection
        println!("Would validate directives for all queries");
        Ok(vec![])
    }
}
