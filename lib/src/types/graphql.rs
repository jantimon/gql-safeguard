use crate::types::directive::Directive;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphQLItem {
    Query(QueryOperation),
    Fragment(FragmentDefinition),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryOperation {
    pub name: String,
    pub fields: Vec<Field>,
    pub fragments: Vec<FragmentSpread>,
    pub directives: Vec<Directive>,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentDefinition {
    pub name: String,
    pub fields: Vec<Field>,
    pub fragments: Vec<FragmentSpread>,
    pub directives: Vec<Directive>,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub directives: Vec<Directive>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentSpread {
    pub name: String,
    pub directives: Vec<Directive>,
}
