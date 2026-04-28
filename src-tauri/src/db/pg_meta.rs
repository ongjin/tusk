// src-tauri/src/db/pg_meta.rs
//
// Per-table metadata lookups (PK columns, enum values, FK targets) with
// LRU cache keyed by (conn_id, schema, table). Implementation lands in
// Task 5.
