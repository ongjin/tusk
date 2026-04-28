import { create } from "zustand";

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
    const rowKey = JSON.stringify(args.pkValues);
    set((state) => {
      const next = new Map(state.byRow);
      const existing = next.get(rowKey);
      if (existing) {
        const editIdx = existing.edits.findIndex(
          (e) => e.column === args.column,
        );
        const updatedEdits =
          editIdx >= 0
            ? existing.edits.map((e, i) =>
                i === editIdx ? { ...e, next: args.next } : e,
              )
            : [
                ...existing.edits,
                {
                  column: args.column,
                  original: args.original,
                  next: args.next,
                },
              ];
        next.set(rowKey, { ...existing, edits: updatedEdits });
      } else {
        const change: PendingChange = {
          rowKey,
          table: args.table,
          pk: { columns: args.pkColumns, values: args.pkValues },
          edits: [
            {
              column: args.column,
              original: args.original,
              next: args.next,
            },
          ],
          op: "update",
          capturedRow: args.capturedRow,
          capturedColumns: args.capturedColumns,
          capturedAt: Date.now(),
        };
        next.set(rowKey, change);
      }
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
