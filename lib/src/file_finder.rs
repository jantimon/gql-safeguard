use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct FileFinder {
    include_set: GlobSet,
    ignore_set: Option<GlobSet>,
}

impl FileFinder {
    pub fn new(pattern: &str, ignore_pattern: Option<&str>) -> Result<Self> {
        let mut include_builder = GlobSetBuilder::new();
        include_builder.add(Glob::new(pattern)?);
        let include_set = include_builder.build()?;

        let ignore_set = if let Some(ignore) = ignore_pattern {
            let mut ignore_builder = GlobSetBuilder::new();
            ignore_builder.add(Glob::new(ignore)?);
            Some(ignore_builder.build()?)
        } else {
            None
        };

        Ok(Self {
            include_set,
            ignore_set,
        })
    }

    pub fn matches_pattern(&self, path: &Path) -> bool {
        if let Some(path_str) = path.to_str() {
            let matches_include = self.include_set.is_match(path_str);
            let matches_ignore = self
                .ignore_set
                .as_ref()
                .map(|ignore_set| ignore_set.is_match(path_str))
                .unwrap_or(false);

            matches_include && !matches_ignore
        } else {
            false
        }
    }

    pub fn find_files(&self, root_path: &Path) -> Result<Vec<PathBuf>> {
        let mut matching_files = Vec::new();

        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && self.matches_pattern(path) {
                matching_files.push(path.to_path_buf());
            }
        }

        Ok(matching_files)
    }
}
