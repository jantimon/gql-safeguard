# Goal: GraphQL Directive Analysis CLI

## Project Overview
Build a blazing fast CLI tool that identifies missing `@catch` directives for `@throwOnFieldError` usage in TypeScript/TSX files to prevent system-wide failures.

## Core Requirements

### Functionality
- Scan all `**/*.ts` and `**/*.tsx` files in parallel
- Extract GraphQL operations from `gql` and `graphql` tagged template literals
- Build global fragment and query registry with file locations
- Create dependency graph showing fragment composition
- Validate that every `@throwOnFieldError` has corresponding `@catch` protection
- `@catch` at query/parent level protects all nested fragments and fields
- Generate human-readable violation reports with file locations and fragment trees

### Performance Constraints
- Use `dashmap` for parallel file processing
- Process file discovery and GraphQL extraction in parallel
- Sequential GraphQL parsing per file (after SWC extraction)
- `FxHashMap` for all lookup operations
- Avoid regex parsing to prevent issues with commented code

## Crate Dependencies

```toml
[dependencies]
globset = "0.4"           # Fast file pattern matching
swc_core = "0.90"         # TypeScript/TSX parsing
swc_ecma_parser = "0.144" # ECMAScript parsing
swc_ecma_ast = "0.110"    # AST types
graphql-parser = "0.4"    # GraphQL syntax parsing
dashmap = "5.5"           # Concurrent HashMap
rustc-hash = "1.1"        # FxHashMap/FxHashSet
rayon = "1.8"             # Parallel iterators
clap = { version = "4.4", features = ["derive"] } # CLI parsing
anyhow = "1.0"            # Error handling
serde = { version = "1.0", features = ["derive"] } # Serialization
```

## Core Data Structures

### GraphQL Types
```rust
// Directive representation
pub enum DirectiveType {
    Catch,
    ThrowOnFieldError,
}

pub struct Directive {
    pub directive_type: DirectiveType,
    pub location: SourceLocation,
}

// GraphQL operations
pub enum GraphQLItem {
    Query(QueryOperation),
    Fragment(FragmentDefinition),
}

pub struct QueryOperation {
    pub name: String,
    pub fields: Vec<Field>,
    pub fragments: Vec<FragmentSpread>,
    pub directives: Vec<Directive>,
    pub file_path: PathBuf,
}

pub struct FragmentDefinition {
    pub name: String,
    pub fields: Vec<Field>,
    pub fragments: Vec<FragmentSpread>,
    pub directives: Vec<Directive>,
    pub file_path: PathBuf,
}

pub struct Field {
    pub name: String,
    pub directives: Vec<Directive>,
}

pub struct FragmentSpread {
    pub name: String,
    pub directives: Vec<Directive>,
}
```

### Registry Types
```rust
// Global registries using FxHashMap for performance
pub struct FragmentRegistry {
    fragments: FxHashMap<String, FragmentDefinition>,
}

pub struct QueryRegistry {
    queries: FxHashMap<String, QueryOperation>,
}
```

### Dependency Graph
```rust
pub struct DependencyGraph {
    // Query name -> Vec<Fragment dependencies>
    query_dependencies: FxHashMap<String, Vec<FragmentNode>>,
}

pub struct FragmentNode {
    pub name: String,
    pub children: Vec<FragmentNode>,
    pub directives: Vec<Directive>,
}
```

### Violation Reporting
```rust
pub struct Violation {
    pub violation_type: ViolationType,
    pub query_name: String,
    pub fragment_path: Vec<String>, // Path from query to problematic fragment
    pub file_location: FileLocation,
    pub message: String,
}

pub enum ViolationType {
    MissingCatch,
    UnprotectedThrowOnFieldError,
}
```

## Core Algorithms

### 1. Parallel File Processing
- Use `globset` to compile file patterns
- Use `dashmap` to store results from parallel processing
- Use `rayon` for parallel file iteration
- Extract GraphQL from each file using SWC
- Parse extracted GraphQL strings sequentially per file

### 2. Registry Building
- Single pass through all extracted GraphQL operations
- Build `FragmentRegistry` and `QueryRegistry` with `FxHashMap`
- Store file locations for each fragment/query
- Handle duplicate names by reporting conflicts

### 3. Dependency Graph Construction
- For each query, recursively resolve fragment dependencies
- Build tree structure showing fragment composition
- Detect circular dependencies
- Store directive information at each level

### 4. Directive Validation
- Traverse dependency graph for each query
- Check if `@throwOnFieldError` is protected by `@catch`
- `@catch` at parent level protects all children
- Report violations with full context path

### 5. Tree Visualization
- Use provided `TreeFormatter` for dependency visualization
- Show fragment hierarchy with directive annotations
- Include in violation reports for debugging

## CLI Interface

### Commands
```bash
# Basic usage
graphql-directive-analyzer [PATH]

# Options
--pattern <PATTERN>     # File glob pattern (default: **/*.{ts,tsx})
--output <FORMAT>       # Output format: text, json (default: text)
--show-trees           # Include dependency trees in output
--verbose              # Show processing details
```

### Output Format
```
Found 3 violations:

Query: GetUserProfile (src/queries/user.ts:15)
├── Fragment: UserDetails
│   └── Field: avatar @throwOnFieldError (src/fragments/user.ts:8)
└── Missing @catch directive

Dependency Tree:
GetUserProfile
├── UserDetails
│   ├── BasicInfo
│   └── Avatar @throwOnFieldError ⚠️
└── Permissions

Fix: Add @catch directive to GetUserProfile or UserDetails fragment
```

## Testing Strategy

### Unit Tests
- Test each component independently
- Mock file system operations
- Test GraphQL parsing with various syntax
- Test directive validation logic

### Integration Tests
- End-to-end CLI testing with fixture files
- Test parallel processing behavior
- Test error handling and edge cases

### Fixtures Structure
```
fixtures/
├── valid/
│   ├── query_with_catch.tsx          # Query with @catch protecting @throwOnFieldError
│   ├── nested_fragments_protected.ts # Deep nesting with proper protection
│   └── fragment_level_catch.ts       # @catch at fragment level
├── invalid/
│   ├── missing_catch.tsx             # @throwOnFieldError without @catch
│   ├── unprotected_nested.ts         # Deep nesting without protection
│   └── partial_protection.ts         # Some fragments protected, others not
└── edge_cases/
    ├── circular_fragments.ts         # Circular fragment dependencies
    ├── commented_graphql.ts          # GraphQL in comments (should be ignored)
    └── dynamic_imports.tsx           # GraphQL in dynamic imports
```

## Implementation Priority

1. **File scanning and GraphQL extraction** - Foundation for all other work
2. **Registry building** - Core data structures
3. **Basic directive validation** - Core functionality
4. **Dependency graph construction** - Enable complex validation
5. **Tree visualization and reporting** - User experience
6. **CLI interface and output formatting** - Polish and usability
7. **Comprehensive testing** - Reliability and edge cases

## Success Criteria

- Accurately identify all `@throwOnFieldError` without corresponding `@catch`
- Handle complex fragment nesting and inheritance
- Provide clear, actionable violation reports with file locations
- Zero false positives from commented or string-literal GraphQL
- Comprehensive test coverage with realistic fixtures