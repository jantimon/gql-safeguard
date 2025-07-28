pub mod file_finder;
pub mod parsers;
pub mod registry;
pub mod registry_to_graph;
pub mod tree_formatter;

pub mod analyzer {
    use std::path::Path;

    pub fn analyze_directory(path: &Path, pattern: &str) {
        println!(
            "Would analyze directory: {} with pattern: {}",
            path.display(),
            pattern
        );
    }
}
