use std::path::PathBuf;

fn main() {
    // Test parsing each edge case file individually
    let files = vec![
        "fixtures/edge_cases/additional-queries.ts",
        "fixtures/edge_cases/circular_fragments.ts", 
        "fixtures/edge_cases/commented_graphql.ts",
        "fixtures/edge_cases/dynamic_imports.tsx",
    ];
    
    for file_path in files {
        println!("\n=== Testing {} ===", file_path);
        
        let extractor = gql_safeguard_lib::scanner::extractor::GraphQLExtractor::new();
        let ast_builder = gql_safeguard_lib::parser::ast_builder::AstBuilder::new();
        
        match extractor.extract_from_file(&PathBuf::from(file_path)) {
            Ok(graphql_strings) => {
                println!("Found {} GraphQL strings", graphql_strings.len());
                
                for (i, graphql_string) in graphql_strings.iter().enumerate() {
                    println!("GraphQL String {}:", i + 1);
                    println!("Content: {}", graphql_string.content);
                    
                    match ast_builder.build_from_graphql_string(graphql_string) {
                        Ok(items) => {
                            println!("✅ Parsed successfully, found {} items", items.len());
                        }
                        Err(e) => {
                            println!("❌ Parse error: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("❌ Extraction error: {}", e);
            }
        }
    }
}