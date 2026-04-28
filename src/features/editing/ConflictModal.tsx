import type { Cell, PendingChange } from "@/lib/types";
import { renderCell } from "@/features/results/cells";
import { usePendingChanges } from "@/store/pendingChanges";

interface Props {
  conflict: { batchId: string; current: Cell[] };
  pending: PendingChange;
  capturedColumns: string[];
  onForceOverwrite: () => void;
  onDiscard: () => void;
  onClose: () => void;
}

export function ConflictModal({
  conflict,
  pending,
  capturedColumns,
  onForceOverwrite,
  onDiscard,
  onClose,
}: Props) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
      onClick={onClose}
    >
      <div
        className="bg-card w-[560px] rounded-sm border p-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-sm font-medium">
          Row was modified by someone else
        </h2>
        <p className="text-muted-foreground mt-1 text-xs">
          The Strict-mode WHERE clause did not match. Choose how to resolve.
        </p>
        <table className="mt-3 w-full text-xs">
          <thead className="text-muted-foreground">
            <tr>
              <th className="text-left">Column</th>
              <th className="text-left">Your edit</th>
              <th className="text-left">Server now</th>
            </tr>
          </thead>
          <tbody>
            {pending.edits.map((e, i) => {
              const colIdx = capturedColumns.indexOf(e.column);
              const serverNow: Cell =
                colIdx >= 0
                  ? conflict.current[colIdx]
                  : { kind: "Unknown", value: { oid: 0, text: "?" } };
              return (
                <tr key={i}>
                  <td className="pr-2">{e.column}</td>
                  <td className="pr-2">{renderCell(e.next)}</td>
                  <td>{renderCell(serverNow)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
        <div className="mt-4 flex justify-end gap-2 text-xs">
          <button
            type="button"
            onClick={onDiscard}
            className="rounded-sm border px-2 py-1"
          >
            Discard your edits
          </button>
          <button
            type="button"
            onClick={() => {
              // Re-edit on top of server: replace capturedRow with current snapshot.
              const next = new Map(usePendingChanges.getState().byRow);
              next.set(pending.rowKey, {
                ...pending,
                capturedRow: conflict.current,
                capturedAt: Date.now(),
              });
              usePendingChanges.setState({ byRow: next });
              onClose();
            }}
            className="rounded-sm border px-2 py-1"
          >
            Re-edit on top of server
          </button>
          <button
            type="button"
            onClick={onForceOverwrite}
            className="rounded-sm border bg-amber-500 px-2 py-1 text-black"
          >
            Force overwrite
          </button>
        </div>
      </div>
    </div>
  );
}
