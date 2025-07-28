pub struct TreeFormatter {
    lines: Vec<(usize, String)>,
    max_depth: usize,
}

impl Default for TreeFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TreeFormatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_tree())
    }
}

impl TreeFormatter {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            max_depth: 0,
        }
    }

    /// Adds a line to the tree structure
    ///
    /// # Arguments
    /// * `depth` - The depth level (0 = root, 1 = first level, etc.)
    /// * `text` - The text content to display
    pub fn add_line(&mut self, depth: usize, text: &str) {
        self.lines.push((depth, text.to_string()));
        self.max_depth = self.max_depth.max(depth);
    }

    /// Adds an entire TreeFormatter as a subtree at the specified depth
    ///
    /// # Arguments
    /// * `depth` - The depth level to add the subtree at
    /// * `tree` - The TreeFormatter to add as a subtree
    pub fn add_tree(&mut self, depth: usize, tree: &TreeFormatter) {
        for (line_depth, text) in &tree.lines {
            let new_depth = depth + line_depth;
            self.lines.push((new_depth, text.clone()));
            self.max_depth = self.max_depth.max(new_depth);
        }
    }

    /// Formats the entire tree structure as a string
    fn format_tree(&self) -> String {
        let mut result = String::new();
        let max_level = self.max_depth + 1;
        let mut last_at_levels = vec![vec![false; self.lines.len()]; max_level];
        for (i, (depth, text)) in self.lines.iter().enumerate() {
            let is_last = self.is_last_sibling(i, *depth);
            last_at_levels[*depth][i] = is_last;
            let formatted_line = self.format_line_internal(*depth, i, text, &last_at_levels);
            result.push_str(&formatted_line);
            if i < self.lines.len() - 1 {
                result.push('\n');
            }
        }

        result
    }

    /// Helper to determine if a line is the last sibling at its level
    fn is_last_sibling(&self, line_index: usize, level: usize) -> bool {
        // Look for the next line at the same level or shallower
        for i in (line_index + 1)..self.lines.len() {
            let next_depth = self.lines[i].0;
            if next_depth < level {
                // Found a shallower line, so this was the last at this level
                return true;
            }
            if next_depth == level {
                // Found another line at the same level, so this is not the last
                return false;
            }
        }
        // No more lines at this level or shallower, so this is the last
        true
    }

    /// Internal method to format a single line using pre-computed last_at_levels
    fn format_line_internal(
        &self,
        depth: usize,
        line_index: usize,
        text: &str,
        last_at_levels: &[Vec<bool>],
    ) -> String {
        let mut result = String::new();

        // Build the indentation prefix for parent levels (depth - 1)
        for level in 1..depth {
            // Check if any ancestor at this level is the last sibling
            let mut use_empty_space = false;

            // Find the most recent ancestor at this level
            for i in (0..=line_index).rev() {
                if self.lines[i].0 == level {
                    if level < last_at_levels.len() && i < last_at_levels[level].len() {
                        use_empty_space = last_at_levels[level][i];
                    }
                    break;
                }
            }

            if use_empty_space {
                result.push_str("    "); // Empty space for completed branches
            } else {
                result.push_str("|   "); // Continuing branch
            }
        }

        // Add the tree symbol for the current level
        if depth > 0 && depth < last_at_levels.len() && line_index < last_at_levels[depth].len() {
            if last_at_levels[depth][line_index] {
                result.push_str("└── ");
            } else {
                result.push_str("├── ");
            }
        }

        result.push_str(text);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_root_item() {
        let mut formatter = TreeFormatter::new();
        formatter.add_line(0, "root item");
        let result = formatter.to_string();
        assert_eq!(result, "root item");
    }

    #[test]
    fn test_simple_tree() {
        let mut formatter = TreeFormatter::new();
        formatter.add_line(0, "root");
        formatter.add_line(1, "child1");
        formatter.add_line(1, "child2");
        let result = formatter.to_string();
        assert_eq!(result, "root\n├── child1\n└── child2");
    }

    #[test]
    fn test_nested_tree() {
        let mut formatter = TreeFormatter::new();
        formatter.add_line(0, "root");
        formatter.add_line(1, "child1");
        formatter.add_line(2, "grandchild1");
        formatter.add_line(2, "grandchild2");
        formatter.add_line(1, "child2");
        let result = formatter.to_string();
        assert_eq!(
            result,
            "root\n├── child1\n|   ├── grandchild1\n|   └── grandchild2\n└── child2"
        );
    }

    #[test]
    fn test_deep_nesting() {
        let mut formatter = TreeFormatter::new();
        formatter.add_line(0, "root");
        formatter.add_line(1, "level1");
        formatter.add_line(2, "level2");
        formatter.add_line(3, "level3");
        formatter.add_line(4, "level4");
        let result = formatter.to_string();
        assert_eq!(
            result,
            "root\n└── level1\n    └── level2\n        └── level3\n            └── level4"
        );
    }

    #[test]
    fn test_complex_tree() {
        let mut formatter = TreeFormatter::new();
        formatter.add_line(0, "root");
        formatter.add_line(1, "branch1");
        formatter.add_line(2, "leaf1");
        formatter.add_line(2, "leaf2");
        formatter.add_line(1, "branch2");
        formatter.add_line(2, "leaf3");
        formatter.add_line(1, "branch3");
        let result = formatter.to_string();
        assert_eq!(result, "root\n├── branch1\n|   ├── leaf1\n|   └── leaf2\n├── branch2\n|   └── leaf3\n└── branch3");
    }

    #[test]
    fn test_add_tree() {
        let mut main_tree = TreeFormatter::new();
        main_tree.add_line(0, "main");
        main_tree.add_line(1, "first");

        let mut subtree = TreeFormatter::new();
        subtree.add_line(0, "sub_root");
        subtree.add_line(1, "sub_child1");
        subtree.add_line(1, "sub_child2");

        main_tree.add_tree(1, &subtree);
        main_tree.add_line(1, "last");

        let result = main_tree.to_string();
        assert_eq!(
            result,
            "main\n├── first\n├── sub_root\n|   ├── sub_child1\n|   └── sub_child2\n└── last"
        );
    }
}
