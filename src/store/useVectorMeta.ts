import { create } from "zustand";

import { listVectorColumns } from "@/lib/tauri";
import type { VectorColumn } from "@/lib/vector/types";

interface State {
  byConn: Record<string, VectorColumn[]>;
  loading: Record<string, boolean>;
  refresh: (connId: string) => Promise<void>;
  hasVectorAt: (
    connId: string,
    schema: string,
    table: string,
    column: string,
  ) => VectorColumn | null;
  vectorColumnsForTable: (
    connId: string,
    schema: string,
    table: string,
  ) => VectorColumn[];
  tableHasVector: (connId: string, schema: string, table: string) => boolean;
}

export const useVectorMeta = create<State>((set, get) => ({
  byConn: {},
  loading: {},
  async refresh(connId) {
    set((s) => ({ loading: { ...s.loading, [connId]: true } }));
    try {
      const cols = await listVectorColumns(connId);
      set((s) => ({
        byConn: { ...s.byConn, [connId]: cols },
        loading: { ...s.loading, [connId]: false },
      }));
    } catch {
      set((s) => ({
        byConn: { ...s.byConn, [connId]: [] },
        loading: { ...s.loading, [connId]: false },
      }));
    }
  },
  hasVectorAt(connId, schema, table, column) {
    const list = get().byConn[connId] ?? [];
    return (
      list.find(
        (c) => c.schema === schema && c.table === table && c.column === column,
      ) ?? null
    );
  },
  vectorColumnsForTable(connId, schema, table) {
    return (get().byConn[connId] ?? []).filter(
      (c) => c.schema === schema && c.table === table,
    );
  },
  tableHasVector(connId, schema, table) {
    return get().vectorColumnsForTable(connId, schema, table).length > 0;
  },
}));
