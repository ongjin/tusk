import { create } from "zustand";

import {
  listColumns,
  listDatabases,
  listSchemas,
  listTables,
} from "@/lib/tauri";
import type { ColumnInfo } from "@/lib/types";

interface CacheEntry<T> {
  state: "idle" | "loading" | "ready" | "error";
  data?: T;
  error?: string;
}

interface SchemaState {
  databases: Record<string, CacheEntry<string[]>>;
  schemas: Record<string, CacheEntry<string[]>>;
  tables: Record<string, CacheEntry<string[]>>; // key = `${connId}:${schema}`
  columns: Record<string, CacheEntry<ColumnInfo[]>>; // key = `${connId}:${schema}:${table}`

  loadDatabases: (connId: string) => Promise<void>;
  loadSchemas: (connId: string) => Promise<void>;
  loadTables: (connId: string, schema: string) => Promise<void>;
  loadColumns: (connId: string, schema: string, table: string) => Promise<void>;
  clear: (connId: string) => void;
}

export const useSchema = create<SchemaState>((set, get) => ({
  // `databases` and `loadDatabases` are reserved for v1.5 when a single
  // connection can browse multiple databases. The Week 2 tree skips this
  // level since each ConnectionRecord encodes one database.
  databases: {},
  schemas: {},
  tables: {},
  columns: {},

  // Reserved for v1.5 — see comment on `databases` field.
  async loadDatabases(connId) {
    if (get().databases[connId]?.state === "ready") return;
    set((s) => ({
      databases: { ...s.databases, [connId]: { state: "loading" } },
    }));
    try {
      const data = await listDatabases(connId);
      set((s) => ({
        databases: { ...s.databases, [connId]: { state: "ready", data } },
      }));
    } catch (e) {
      set((s) => ({
        databases: {
          ...s.databases,
          [connId]: { state: "error", error: (e as Error).message },
        },
      }));
    }
  },

  async loadSchemas(connId) {
    if (get().schemas[connId]?.state === "ready") return;
    set((s) => ({ schemas: { ...s.schemas, [connId]: { state: "loading" } } }));
    try {
      const data = await listSchemas(connId);
      set((s) => ({
        schemas: { ...s.schemas, [connId]: { state: "ready", data } },
      }));
    } catch (e) {
      set((s) => ({
        schemas: {
          ...s.schemas,
          [connId]: { state: "error", error: (e as Error).message },
        },
      }));
    }
  },

  async loadTables(connId, schema) {
    const key = `${connId}:${schema}`;
    if (get().tables[key]?.state === "ready") return;
    set((s) => ({ tables: { ...s.tables, [key]: { state: "loading" } } }));
    try {
      const data = await listTables(connId, schema);
      set((s) => ({
        tables: { ...s.tables, [key]: { state: "ready", data } },
      }));
    } catch (e) {
      set((s) => ({
        tables: {
          ...s.tables,
          [key]: { state: "error", error: (e as Error).message },
        },
      }));
    }
  },

  async loadColumns(connId, schema, table) {
    const key = `${connId}:${schema}:${table}`;
    if (get().columns[key]?.state === "ready") return;
    set((s) => ({ columns: { ...s.columns, [key]: { state: "loading" } } }));
    try {
      const data = await listColumns(connId, schema, table);
      set((s) => ({
        columns: { ...s.columns, [key]: { state: "ready", data } },
      }));
    } catch (e) {
      set((s) => ({
        columns: {
          ...s.columns,
          [key]: { state: "error", error: (e as Error).message },
        },
      }));
    }
  },

  clear(connId) {
    set((s) => {
      const databases = { ...s.databases };
      const schemas = { ...s.schemas };
      const tables = { ...s.tables };
      const columns = { ...s.columns };
      delete databases[connId];
      delete schemas[connId];
      Object.keys(tables).forEach(
        (k) => k.startsWith(`${connId}:`) && delete tables[k],
      );
      Object.keys(columns).forEach(
        (k) => k.startsWith(`${connId}:`) && delete columns[k],
      );
      return { databases, schemas, tables, columns };
    });
  },
}));
