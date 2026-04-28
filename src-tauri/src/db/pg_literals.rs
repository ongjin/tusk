// src-tauri/src/db/pg_literals.rs
//
// Renders typed values as PG literal SQL fragments (single-quoted strings,
// hex bytea, NULL, etc.) — used to build human-readable preview SQL that
// matches what the parameterized executor will actually run. Implementation
// lands in Task 2.
