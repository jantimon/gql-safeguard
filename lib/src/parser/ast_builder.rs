use crate::scanner::extractor::GraphQLString;
use crate::types::directive::{Directive, DirectiveType, SourceLocation};
use crate::types::graphql::{
    Field, FragmentDefinition, FragmentSpread, GraphQLItem, QueryOperation,
};
use anyhow::Result;
use graphql_parser::parse_query;
use graphql_parser::query::{
    Definition, Document as QueryDocument, OperationDefinition, Selection, SelectionSet,
};
use graphql_parser::schema::Document;

pub struct AstBuilder;

impl AstBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build_from_graphql_string(
        &self,
        graphql_string: &GraphQLString,
    ) -> Result<Vec<GraphQLItem>> {
        let document: QueryDocument<String> = parse_query(&graphql_string.content)?;
        let mut items = Vec::new();

        for definition in document.definitions {
            match definition {
                Definition::Operation(op) => {
                    if let Some(query) = self.convert_operation(op, &graphql_string.file_path)? {
                        items.push(GraphQLItem::Query(query));
                    }
                }
                Definition::Fragment(frag) => {
                    let fragment = self.convert_fragment(frag, &graphql_string.file_path)?;
                    items.push(GraphQLItem::Fragment(fragment));
                }
            }
        }

        Ok(items)
    }

    fn convert_operation(
        &self,
        op: OperationDefinition<String>,
        file_path: &std::path::Path,
    ) -> Result<Option<QueryOperation>> {
        match op {
            OperationDefinition::Query(query) => {
                let name = query.name.unwrap_or_else(|| "AnonymousQuery".to_string());
                let directives = self.convert_directives(&query.directives);
                let (fields, fragments) = self.convert_selection_set(&query.selection_set);

                Ok(Some(QueryOperation {
                    name,
                    fields,
                    fragments,
                    directives,
                    file_path: file_path.to_path_buf(),
                }))
            }
            OperationDefinition::Mutation(_) | OperationDefinition::Subscription(_) => {
                // Skip mutations and subscriptions for now
                Ok(None)
            }
            OperationDefinition::SelectionSet(_) => {
                // Skip bare selection sets for now
                Ok(None)
            }
        }
    }

    fn convert_fragment(
        &self,
        frag: graphql_parser::query::FragmentDefinition<String>,
        file_path: &std::path::Path,
    ) -> Result<FragmentDefinition> {
        let directives = self.convert_directives(&frag.directives);
        let (fields, fragments) = self.convert_selection_set(&frag.selection_set);

        Ok(FragmentDefinition {
            name: frag.name,
            fields,
            fragments,
            directives,
            file_path: file_path.to_path_buf(),
        })
    }

    fn convert_selection_set(
        &self,
        selection_set: &SelectionSet<String>,
    ) -> (Vec<Field>, Vec<FragmentSpread>) {
        let mut fields = Vec::new();
        let mut fragments = Vec::new();

        for selection in &selection_set.items {
            match selection {
                Selection::Field(field) => {
                    let directives = self.convert_directives(&field.directives);
                    fields.push(Field {
                        name: field.name.clone(),
                        directives,
                    });
                }
                Selection::FragmentSpread(spread) => {
                    let directives = self.convert_directives(&spread.directives);
                    fragments.push(FragmentSpread {
                        name: spread.fragment_name.clone(),
                        directives,
                    });
                }
                Selection::InlineFragment(_) => {
                    // Skip inline fragments for now
                }
            }
        }

        (fields, fragments)
    }

    fn convert_directives(
        &self,
        directives: &[graphql_parser::query::Directive<String>],
    ) -> Vec<Directive> {
        directives
            .iter()
            .filter_map(|dir| {
                let directive_type = match dir.name.as_str() {
                    "catch" => DirectiveType::Catch,
                    "throwOnFieldError" => DirectiveType::ThrowOnFieldError,
                    _ => return None,
                };

                Some(Directive {
                    directive_type,
                    location: SourceLocation { line: 1, column: 1 }, // Placeholder
                })
            })
            .collect()
    }
}
