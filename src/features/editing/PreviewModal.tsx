import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { Cell, PendingChange } from "@/lib/types";
import { usePendingChanges } from "@/store/pendingChanges";
import { useSettings } from "@/store/settings";

interface BatchOk {
  status: "ok";
  batchId: string;
  executedSql: string;
  affected: number;
}
interface BatchConflict {
  status: "conflict";
  batchId: string;
  executedSql: string;
  current: Cell[];
}
interface BatchError {
  status: "error";
  batchId: string;
  executedSql: string;
  message: string;
}
export type BatchResult = BatchOk | BatchConflict | BatchError;

interface Props {
  connId: string;
  onClose: () => void;
  onSubmitDone: (r: BatchResult[]) => void;
}

export function PreviewModal({ connId, onClose, onSubmitDone }: Props) {
  const list = usePendingChanges((s) => s.list());
  const mode = useSettings((s) => s.editConflictMode);
  const [previews, setPreviews] = useState<BatchResult[]>([]);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    let cancelled = false;
    invoke<BatchResult[]>("preview_pending_changes", {
      batches: list.map(toRustBatch),
      mode,
    })
      .then((r) => {
        if (!cancelled) setPreviews(r);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [list, mode]);

  const submit = async () => {
    if (submitting) return;
    setSubmitting(true);
    try {
      const r = await invoke<{ batches: BatchResult[] }>(
        "submit_pending_changes",
        {
          connectionId: connId,
          batches: list.map(toRustBatch),
          mode,
        },
      );
      onSubmitDone(r.batches);
    } finally {
      setSubmitting(false);
    }
  };

  const previewSql = previews
    .map((p) => p.executedSql)
    .filter(Boolean)
    .join(";\n\n");

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
      onClick={onClose}
    >
      <div
        className="bg-card max-h-[80vh] w-[640px] overflow-auto rounded-sm border p-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-sm font-medium">
          Preview pending changes ({list.length})
        </h2>
        <pre className="mt-2 text-xs whitespace-pre-wrap">{previewSql}</pre>
        <p className="text-muted-foreground mt-2 text-xs">
          Actual execution uses parameterized binds; this rendering inlines
          literals using PG escape rules.
        </p>
        <div className="mt-3 flex justify-end gap-2 text-xs">
          <button
            onClick={onClose}
            className="rounded-sm border px-2 py-1"
            disabled={submitting}
          >
            Cancel
          </button>
          <button
            onClick={submit}
            disabled={submitting || list.length === 0}
            className="rounded-sm border bg-amber-500 px-2 py-1 text-black disabled:opacity-50"
          >
            {submitting ? "Submitting…" : "Submit Now"}
          </button>
        </div>
      </div>
    </div>
  );
}

function toRustBatch(p: PendingChange): unknown {
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
