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
  /**
   * Optional explicit rowKey override. Used by ghost (insert) rows whose
   * `pkValues` are empty — `JSON.stringify([])` would collide on `"[]"`,
   * so the inserted row carries its own synthetic key (`__insert_…`).
   */
  rowKey?: string;
}

interface InsertRowArgs {
  table: { schema: string; name: string };
  pkColumns: string[];
  defaults: Record<string, Cell>;
  capturedColumns: string[];
}

interface DeleteRowArgs {
  table: { schema: string; name: string };
  pkColumns: string[];
  pkValues: Cell[];
  capturedRow: Cell[];
  capturedColumns: string[];
}

interface PendingChangesStore {
  byRow: Map<string, PendingChange>;
  upsertEdit(args: UpsertEditArgs): void;
  insertRow(args: InsertRowArgs): void;
  deleteRow(args: DeleteRowArgs): void;
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
      const rowKey = args.rowKey ?? JSON.stringify(args.pkValues);
      const existing = next.get(rowKey);
      // For ghost rows (op === "insert"), edits store the staged column values
      // directly; we update in place so the insert payload reflects the latest
      // user input without flipping the op back to "update".
      if (existing && existing.op === "insert") {
        const idx = existing.edits.findIndex((e) => e.column === args.column);
        const editEntry = {
          column: args.column,
          original: args.original,
          next: args.next,
        };
        if (idx >= 0) {
          existing.edits[idx] = editEntry;
        } else {
          existing.edits.push(editEntry);
        }
        // Keep capturedRow in sync so build_insert can read defaults from it.
        const colIdx = existing.capturedColumns.indexOf(args.column);
        if (colIdx >= 0) {
          existing.capturedRow = existing.capturedRow.slice();
          existing.capturedRow[colIdx] = args.next;
        }
        next.set(rowKey, existing);
        return { byRow: next };
      }
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
  insertRow({ table, pkColumns, defaults, capturedColumns }) {
    set((s) => {
      const next = new Map(s.byRow);
      const rowKey = `__insert_${Math.random().toString(36).slice(2, 8)}`;
      const change: PendingChange = {
        rowKey,
        table,
        pk: { columns: pkColumns, values: [] },
        edits: capturedColumns.map((col) => ({
          column: col,
          original: { kind: "Null" },
          next: defaults[col] ?? { kind: "Null" },
        })),
        op: "insert",
        capturedRow: capturedColumns.map(
          (col) => defaults[col] ?? { kind: "Null" },
        ),
        capturedColumns,
        capturedAt: Date.now(),
      };
      next.set(rowKey, change);
      return { byRow: next };
    });
  },
  deleteRow({ table, pkColumns, pkValues, capturedRow, capturedColumns }) {
    set((s) => {
      const next = new Map(s.byRow);
      const rowKey = JSON.stringify(pkValues);
      next.set(rowKey, {
        rowKey,
        table,
        pk: { columns: pkColumns, values: pkValues },
        edits: [],
        op: "delete",
        capturedRow,
        capturedColumns,
        capturedAt: Date.now(),
      });
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
