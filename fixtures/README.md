# Test Fixtures

Minimal test cases for GraphQL directive analysis.

## Valid Cases (✅ Should pass)

### `valid/query_with_catch.tsx`
- Simple query with `@catch` at query level protecting `@throwOnFieldError`

### `valid/nested_fragments_protected.ts`  
- Fragment composition where `@catch` on fragment protects nested `@throwOnFieldError`

### `valid/fragment_level_catch.ts`
- Multiple `@throwOnFieldError` directives protected by fragment-level `@catch`

## Invalid Cases (❌ Should fail)

### `invalid/missing_catch.tsx`
- Query with `@throwOnFieldError` but no `@catch` protection

### `invalid/unprotected_nested.ts`
- Fragment with `@throwOnFieldError` used in query without any `@catch`

### `invalid/partial_protection.ts`
- Mixed scenario: one fragment protected, another unprotected

## Edge Cases (🧪 Complex scenarios)

### `edge_cases/circular_fragments.ts`
- Circular fragment references (should detect cycle)

### `edge_cases/commented_graphql.ts`  
- GraphQL in comments should be ignored
- Only active GraphQL should be analyzed

### `edge_cases/dynamic_imports.tsx`
- GraphQL in dynamic imports and template literals
- Tests complex extraction scenarios

### `edge_cases/additional-queries.ts`
- Support file for dynamic imports test
- Contains both valid and invalid patterns

## Expected Results

When running the analyzer:
- `fixtures/valid/` → 0 violations
- `fixtures/invalid/` → 3 violations  
- `fixtures/edge_cases/` → 2 violations (dynamic_imports.tsx, additional-queries.ts)