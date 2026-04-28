import { useState } from "react";
import { toast } from "sonner";

import { PendingBadge } from "@/features/editing/PendingBadge";
import {
  PreviewModal,
  type BatchResult,
} from "@/features/editing/PreviewModal";
import type { QueryResult } from "@/lib/types";
import { usePendingChanges } from "@/store/pendingChanges";

interface Props {
  result?: QueryResult;
  error?: string;
  busy?: boolean;
  connId?: string | null;
}

export function ResultsHeader({ result, error, busy, connId }: Props) {
  const [showPreview, setShowPreview] = useState(false);

  const handleSubmitDone = (batches: BatchResult[]) => {
    setShowPreview(false);
    const conflicts = batches.filter((b) => b.status === "conflict");
    const errors = batches.filter((b) => b.status === "error");
    const oks = batches.filter((b) => b.status === "ok");
    if (conflicts.length > 0) {
      toast.warning(
        `${conflicts.length} conflict(s); ${oks.length} pending stays open. (ConflictModal arrives in Task 18)`,
      );
      // Leave pending changes intact so the user can retry/discard.
    } else if (errors.length > 0) {
      const err = errors[0] as { message: string };
      toast.error(`Submit failed: ${err.message}`);
      // Leave pending intact.
    } else {
      toast.success(`${oks.length} row(s) updated`);
      usePendingChanges.getState().revertAll();
    }
  };

  return (
    <div className="border-border bg-muted/40 flex items-center gap-3 border-b px-3 py-1.5 text-xs">
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
          <PendingBadge
            onPreview={() => setShowPreview(true)}
            onSubmit={() => setShowPreview(true)}
            onRevert={() => usePendingChanges.getState().revertAll()}
          />
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
    </div>
  );
}
