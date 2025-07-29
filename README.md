# GQL Safeguard

A Rust-powered static analysis tool that prevents GraphQL runtime errors by enforcing proper `@catch` directive protection for `@throwOnFieldError` and `@required(action: THROW)` usage in Relay applications

![gql-guard](https://github.com/user-attachments/assets/96ee62ce-c0b1-4c40-9fe5-59b7d492c6d9)

## Why GQL Safeguard?

When using `@throwOnFieldError` or `@required(action: THROW)` in GraphQL queries, field errors are converted to exceptions that bubble up through your component tree. Without proper `@catch` directive protection, these exceptions can crash entire pages or app sections.

The reason why `@catch` is enforced instead of React Error Boundaries: Error boundaries don't work during SSR, but `@catch` does. This makes `@catch` essential for server-side rendered applications.

GQL Safeguard analyzes your TypeScript/TSX codebase to ensure every `@throwOnFieldError` directive and every `@required(action: THROW)` directive is properly protected by a `@catch` directive in an ancestor field or fragment.

## Key Features

- **🚀 Fast & Efficient**: Written in Rust with parallel processing for large codebases
- **🎯 Precise Analysis**: Uses AST parsing instead of regex to avoid false positives
- **🌳 Fragment Resolution**: Fully resolves fragment dependencies for accurate validation  
- **📊 Clear Error Reporting**: Rich error messages with visual query structure
- **⚡ Memory Efficient**: Streaming file processing prevents memory issues
- **🔧 CLI & Library**: Use as a command-line tool or integrate into your build pipeline

## Installation

```bash
# Install globally via npm
npm install -g gql-safeguard

# Or use with npx (no installation required)
npx gql-safeguard --help
```

## Quick Start

### Basic Usage

```bash
# Validate all GraphQL in current directory
npx gql-safeguard . validate

# Validate specific patterns
npx gql-safeguard src/ validate --pattern "**/*.{ts,tsx}"

# Show detailed processing information
npx gql-safeguard . validate --verbose

# Export GraphQL registry for external analysis  
npx gql-safeguard . json > graphql-analysis.json
```

### Example Validation

**❌ Invalid - Unprotected directives:**

**user-query.ts:**
```typescript
const query = gql`
  query MyQuery {
    user {
      ...UserProfile  # ❌ Fragment contains unprotected directives!
    }
  }
`;
```

**user-profile-fragment.ts:**
```typescript
const fragment = gql`
  fragment UserProfile on User {
    name @throwOnFieldError           # ❌ No @catch protection!
    email @required(action: THROW)    # ❌ No @catch protection!
  }
`;
```

**✅ Valid - Properly Protected:**

**user-query.ts:**
```typescript
const query = gql`
  query MyQuery {
    user @catch {  # ✅ Catches errors from fragment
      ...UserProfile
    }
  }
`;
```

**user-profile-fragment.ts:**
```typescript
const fragment = gql`
  fragment UserProfile on User {
    name @throwOnFieldError           # ✅ Protected by ancestor @catch in query
    email @required(action: THROW)    # ✅ Protected by ancestor @catch in query
  }
`;
```

## CLI Reference

### Commands

#### `validate`
Validates GraphQL operations for proper `@catch` directive protection.

```bash
npx gql-safeguard [PATH] validate [OPTIONS]
```

**Options:**
- `--show-trees`: Display fragment dependency trees in output
- `--verbose`: Show detailed processing information
- `--pattern <GLOB>`: File pattern to match (default: `**/*.{ts,tsx}`)
- `--ignore <GLOB>`: Files to ignore (default: node_modules, .git, etc.)
- `--cwd <PATH>`: Change working directory

#### `json`
Export extracted GraphQL registry in JSON format for external analysis.

```bash
npx gql-safeguard [PATH] json [OPTIONS]
```

### Configuration

GQL Safeguard automatically ignores common build artifacts:
- `**/node_modules`
- `**/.git`
- `**/.yarn`
- `**/.swc`
- `**/*.xcassets`

Override with `--ignore` flag for custom patterns.

## How It Works

GQL Safeguard uses a sophisticated multi-stage analysis pipeline:

### 1. **TypeScript Extraction**
Uses SWC AST parsing to extract GraphQL from `gql` and `graphql` tagged template literals, avoiding false positives from comments or dynamic content.

### 2. **GraphQL Parsing**
Converts extracted GraphQL strings into structured AST representations with full directive extraction and position tracking.

### 3. **Fragment Resolution**
Expands fragment spreads (`...FragmentName`) into complete dependency trees while preserving directive inheritance relationships.

### 4. **Protection Validation**
Validates that every `@throwOnFieldError` directive and every `@required(action: THROW)` directive has proper `@catch` ancestor protection using single-pass recursive traversal with O(n) complexity.

## Validation Rules

### Rule 1: Protection Requirement
Every `@throwOnFieldError` directive and every `@required(action: THROW)` directive must be protected by at least one `@catch` directive in an ancestor field, fragment, or query.

### Rule 2: Useful Protection  
Every `@catch` directive must protect at least one `@throwOnFieldError` or `@required(action: THROW)` directive in its subtree to avoid unnecessary error handling.

### Rule 3: Required Action Filtering
Only `@required` directives with `action: THROW` are validated. Other action values (`LOG`, `WARN`, `NONE`) or missing action arguments are ignored as they don't throw exceptions.

## Ignoring Specific Fields

You can disable validation for specific fields by placing the `gql-safeguard-ignore` comment in the line before the field:

```graphql
query GetUser {
  user @catch {
    name @throwOnFieldError           # ✅ Protected by @catch
    
    # gql-safeguard-ignore
    email @throwOnFieldError          # ⏭️ Ignored by gql-safeguard
    
    profile {
      # gql-safeguard-ignore  
      avatar @required(action: THROW) # ⏭️ Ignored by gql-safeguard
      bio @required(action: THROW)    # ✅ Still validated (protected by @catch)
    }
  }
}
```

## Error Types

### Unprotected Throwing Directives
**Risk**: Field errors from `@throwOnFieldError` or `@required(action: THROW)` will propagate as unhandled exceptions, potentially crashing the page.

**Fix**: Add `@catch` to a parent field or fragment:
```graphql
user @catch {
  profile {
    email @throwOnFieldError         # Now protected
    name @required(action: THROW)    # Now protected
  }
}
```

### Empty @catch  
**Issue**: `@catch` directive doesn't protect any `@throwOnFieldError` or `@required(action: THROW)` directives

**Fix**: Either add throwing directives to descendant fields or remove unnecessary `@catch`

## Development

### Building
```bash
# Build the project
cargo build --release

# Run tests
cargo test

# Update test snapshots
cargo insta review
```

### Architecture

- **`cli/`**: Command-line interface and argument parsing
- **`lib/src/parsers/`**: TypeScript and GraphQL parsing logic
- **`lib/src/registry.rs`**: Concurrent GraphQL extraction and storage
- **`lib/src/registry_to_graph.rs`**: Fragment dependency resolution
- **`lib/src/validate.rs`**: Core validation logic with error reporting
- **`lib/src/tree_formatter.rs`**: Visual tree output formatting
- **`fixtures/`**: Test cases for validation scenarios

### Testing

The project uses comprehensive snapshot testing to ensure consistent behavior:

- **Valid fixtures**: Well-formed GraphQL with proper protection
- **Invalid fixtures**: GraphQL violating protection rules  
- **Edge cases**: Complex scenarios like circular dependencies

## Performance

- **Parallel Processing**: Concurrent file analysis using Rayon
- **Memory Efficient**: Streaming approach prevents memory issues on large codebases
- **Fast Parsing**: SWC-based TypeScript parsing with early exits
- **Optimized Traversal**: Single-pass validation with O(n) complexity

## Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature-name`
3. Add tests for your changes
4. Run the test suite: `cargo test`
5. Submit a pull request

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

---

**Prevent GraphQL runtime errors before they reach production.** 🛡️
