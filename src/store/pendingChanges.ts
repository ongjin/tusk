import { create } from "zustand";

import { cellsEqual } from "@/features/editing/cellSerde";
import type { Cell, PendingChange, ResultMeta } from "@/lib/types";

interface UpsertEditArgs {
  table: { schema: string; name: string };
  pkColumns: string[];
  pkValues: Cell[];
  column: string;
  original: Cell;
  next: Cell;
  capturedRow: Cell[];
  capturedColumns: string[];
}

interface PendingChangesStore {
  byRow: Map<string, PendingChange>;
  upsertEdit(args: UpsertEditArgs): void;
  revertRow(rowKey: string): void;
  revertAll(): void;
  list(): PendingChange[];
  count(): number;
}

export const usePendingChanges = create<PendingChangesStore>((set, get) => ({
  byRow: new Map(),
  upsertEdit(args) {
    set((s) => {
      const next = new Map(s.byRow);
      const rowKey = JSON.stringify(args.pkValues);
      const existing = next.get(rowKey);
      // capturedRow/capturedColumns are snapshot-at-first-edit semantics:
      // optimistic-concurrency relies on the row state when the user started
      // editing, not the latest server state. We deliberately don't refresh
      // these on subsequent edits to the same row.
      const change: PendingChange = existing ?? {
        rowKey,
        table: args.table,
        pk: { columns: args.pkColumns, values: args.pkValues },
        edits: [],
        op: "update",
        capturedRow: args.capturedRow,
        capturedColumns: args.capturedColumns,
        capturedAt: Date.now(),
      };
      const idx = change.edits.findIndex((e) => e.column === args.column);

      // If new value equals original, remove this column's edit (no-op).
      if (cellsEqual(args.next, args.original)) {
        if (idx >= 0) {
          change.edits.splice(idx, 1);
        }
        // If no edits remain on this row, drop it from the map.
        if (change.edits.length === 0) {
          next.delete(rowKey);
        } else {
          next.set(rowKey, change);
        }
        return { byRow: next };
      }

      const editEntry = {
        column: args.column,
        original: args.original,
        next: args.next,
      };
      if (idx >= 0) {
        change.edits[idx] = editEntry;
      } else {
        change.edits.push(editEntry);
      }
      next.set(rowKey, change);
      return { byRow: next };
    });
  },
  revertRow(rowKey) {
    set((state) => {
      const next = new Map(state.byRow);
      next.delete(rowKey);
      return { byRow: next };
    });
  },
  revertAll() {
    set({ byRow: new Map() });
  },
  list() {
    return Array.from(get().byRow.values());
  },
  count() {
    return get().byRow.size;
  },
}));

export function pkValuesOf(meta: ResultMeta, row: Cell[]): Cell[] {
  return meta.pkColumnIndices.map((i) => row[i]);
}
