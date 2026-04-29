import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import { ConflictModal } from "@/features/editing/ConflictModal";
import { PendingBadge } from "@/features/editing/PendingBadge";
import {
  PreviewModal,
  type BatchConflict,
  type BatchResult,
} from "@/features/editing/PreviewModal";
import { toRustBatch } from "@/features/editing/toRustBatch";
import { ExportDialog } from "@/features/export/ExportDialog";
import type { QueryResult } from "@/lib/types";
import { usePendingChanges } from "@/store/pendingChanges";
import { useSettings } from "@/store/settings";

interface Props {
  result?: QueryResult;
  error?: string;
  busy?: boolean;
  connId?: string | null;
  hasPlan?: boolean;
  resultMode?: "rows" | "plan";
  onModeChange?: (mode: "rows" | "plan") => void;
}

export function ResultsHeader({
  result,
  error,
  busy,
  connId,
  hasPlan,
  resultMode,
  onModeChange,
}: Props) {
  const [showPreview, setShowPreview] = useState(false);
  const [showExport, setShowExport] = useState(false);
  const [activeConflict, setActiveConflict] = useState<BatchConflict | null>(
    null,
  );
  const conflictMode = useSettings((s) => s.editConflictMode);
  const setConflictMode = useSettings((s) => s.setEditConflictMode);

  const handleSubmitDone = (batches: BatchResult[]) => {
    setShowPreview(false);
    const conflicts = batches.filter(
      (b) => b.status === "conflict",
    ) as BatchConflict[];
    const errors = batches.filter((b) => b.status === "error");
    const oks = batches.filter((b) => b.status === "ok");
    if (conflicts.length > 0) {
      setActiveConflict(conflicts[0]); // resolve one at a time
    } else if (errors.length > 0) {
      const err = errors[0] as { message: string };
      toast.error(`Submit failed: ${err.message}`);
    } else {
      toast.success(`${oks.length} row(s) updated`);
      usePendingChanges.getState().revertAll();
    }
  };

  const conflictPending = activeConflict
    ? usePendingChanges
        .getState()
        .list()
        .find((p) => p.rowKey === activeConflict.batchId)
    : null;

  return (
    <div className="border-border bg-muted/40 flex items-center gap-3 border-b px-3 py-1.5 text-xs">
      {(result || hasPlan) && (
        <div className="border-input flex overflow-hidden rounded-sm border text-[11px]">
          <button
            type="button"
            onClick={() => onModeChange?.("rows")}
            className={`px-2 py-0.5 ${resultMode === "rows" ? "bg-accent text-accent-foreground" : ""}`}
            disabled={!result}
          >
            Rows
          </button>
          <button
            type="button"
            onClick={() => onModeChange?.("plan")}
            className={`px-2 py-0.5 ${resultMode === "plan" ? "bg-accent text-accent-foreground" : ""}`}
            disabled={!hasPlan}
          >
            Plan
          </button>
        </div>
      )}
      {busy && <span className="text-muted-foreground">Running…</span>}
      {!busy && error && <span className="text-destructive">{error}</span>}
      {!busy && result && (
        <>
          <span>{result.rowCount} rows</span>
          <span className="text-muted-foreground">·</span>
          <span>{result.durationMs} ms</span>
          <span className="text-muted-foreground">·</span>
          {result.meta.editable ? (
            <span
              title={`Editable — ${result.meta.table?.schema}.${result.meta.table?.name}`}
              className="text-xs text-amber-500"
            >
              ✏️
            </span>
          ) : (
            <span
              title={`Read-only — ${result.meta.reason ?? "unknown"}`}
              className="text-muted-foreground text-xs"
            >
              🔒
            </span>
          )}
          {result.meta.editable && (
            <select
              value={conflictMode}
              onChange={(e) =>
                setConflictMode(e.target.value as "pkOnly" | "strict")
              }
              className="bg-background border-input rounded-sm border px-2 py-0.5 text-xs"
              title="Conflict detection mode"
            >
              <option value="pkOnly">PK only</option>
              <option value="strict">Strict</option>
            </select>
          )}
          {result.meta.editable && result.meta.table && (
            <button
              type="button"
              onClick={() =>
                usePendingChanges.getState().insertRow({
                  table: result.meta.table!,
                  pkColumns: result.meta.pkColumns,
                  defaults: {},
                  capturedColumns: result.meta.columnTypes.map((c) => c.name),
                })
              }
              className="border-input hover:bg-accent rounded-sm border px-2 py-0.5 text-xs"
              title="Insert a new row"
            >
              + Row
            </button>
          )}
          <PendingBadge
            onPreview={() => setShowPreview(true)}
            onSubmit={() => setShowPreview(true)}
            onRevert={() => usePendingChanges.getState().revertAll()}
          />
          {result.rows.length > 0 && (
            <button
              type="button"
              onClick={() => setShowExport(true)}
              className="border-input hover:bg-accent rounded-sm border px-2 py-0.5 text-xs"
              title="Export this result"
            >
              Export
            </button>
          )}
        </>
      )}
      {!busy && !result && !error && (
        <span className="text-muted-foreground">
          No result yet — Cmd+Enter to run.
        </span>
      )}
      {showPreview && connId && (
        <PreviewModal
          connId={connId}
          onClose={() => setShowPreview(false)}
          onSubmitDone={handleSubmitDone}
        />
      )}
      {showExport && result && (
        <ExportDialog
          rows={result.rows}
          meta={result.meta}
          onClose={() => setShowExport(false)}
        />
      )}
      {activeConflict && conflictPending && connId && (
        <ConflictModal
          conflict={activeConflict}
          pending={conflictPending}
          capturedColumns={conflictPending.capturedColumns}
          onForceOverwrite={async () => {
            // Re-submit just this batch with mode=pkOnly.
            const r = await invoke<{ batches: BatchResult[] }>(
              "submit_pending_changes",
              {
                connectionId: connId,
                batches: [toRustBatch(conflictPending)],
                mode: "pkOnly",
              },
            );
            setActiveConflict(null);
            handleSubmitDone(r.batches);
          }}
          onDiscard={() => {
            usePendingChanges.getState().revertRow(conflictPending.rowKey);
            setActiveConflict(null);
          }}
          onClose={() => setActiveConflict(null)}
        />
      )}
    </div>
  );
}
