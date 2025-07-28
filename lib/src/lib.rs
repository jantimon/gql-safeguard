pub mod extraction;
pub mod graph;
pub mod registry;
pub mod scanner;
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
