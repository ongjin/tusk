import { create } from "zustand";

import type { SchemaIndexProgress } from "@/lib/types";

interface SchemaIndexState {
  byConn: Record<string, SchemaIndexProgress>;
  set: (p: SchemaIndexProgress) => void;
  clear: (connId: string) => void;
}

export const useSchemaIndex = create<SchemaIndexState>((set) => ({
  byConn: {},
  set: (p) => set((s) => ({ byConn: { ...s.byConn, [p.connId]: p } })),
  clear: (connId) =>
    set((s) => {
      const next = { ...s.byConn };
      delete next[connId];
      return { byConn: next };
    }),
}));
