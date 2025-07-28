use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct FileFinder {
    glob_set: GlobSet,
}

impl FileFinder {
    pub fn new(pattern: &str) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new(pattern)?);
        let glob_set = builder.build()?;
        Ok(Self { glob_set })
    }

    pub fn find_files(&self, root_path: &Path) -> Result<Vec<PathBuf>> {
        let mut matching_files = Vec::new();

        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if let Some(path_str) = path.to_str() {
                    if self.glob_set.is_match(path_str) {
                        matching_files.push(path.to_path_buf());
                    }
                }
            }
        }

        Ok(matching_files)
    }
}
