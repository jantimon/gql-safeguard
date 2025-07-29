//! Two-stage parsing: extract GraphQL from TypeScript, then parse to AST
//!
//! Separation enables robust extraction from complex TS/TSX without GraphQL syntax errors.

pub mod graphql_parser;
pub mod typescript_parser;
