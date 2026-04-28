import type { PendingChange } from "@/lib/types";

/// Translate a `PendingChange` into the camelCase shape the Rust
/// `submit_pending_changes` / `preview_pending_changes` commands expect.
export function toRustBatch(p: PendingChange): unknown {
  return {
    batchId: p.rowKey,
    op: p.op,
    table: p.table,
    pkColumns: p.pk.columns,
    pkValues: p.pk.values,
    edits: p.edits.map((e) => ({ column: e.column, next: e.next })),
    capturedRow: p.capturedRow,
    capturedColumns: p.capturedColumns,
  };
}
