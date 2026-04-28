import { create } from "zustand";

import type { QueryResult } from "@/lib/types";

export interface Tab {
  id: string;
  title: string;
  connectionId: string | null;
  sql: string;
  dirty: boolean;
  lastResult?: QueryResult;
  lastError?: string;
  busy?: boolean;
}

interface TabsState {
  tabs: Tab[];
  activeId: string;
  newTab: (connectionId: string | null) => string;
  closeTab: (id: string) => void;
  updateSql: (id: string, sql: string) => void;
  setActive: (id: string) => void;
  bindConnection: (id: string, connectionId: string | null) => void;
  setResult: (id: string, result: QueryResult) => void;
  setError: (id: string, message: string) => void;
  setBusy: (id: string, busy: boolean) => void;
}

let counter = 1;

const initialId = crypto.randomUUID();
const initialTab: Tab = {
  id: initialId,
  title: `Untitled ${counter++}`,
  connectionId: null,
  sql: "SELECT 1",
  dirty: false,
};

export const useTabs = create<TabsState>((set) => ({
  tabs: [initialTab],
  activeId: initialId,

  newTab(connectionId) {
    const id = crypto.randomUUID();
    set((s) => ({
      tabs: [
        ...s.tabs,
        {
          id,
          title: `Untitled ${counter++}`,
          connectionId,
          sql: "",
          dirty: false,
        },
      ],
      activeId: id,
    }));
    return id;
  },

  closeTab(id) {
    set((s) => {
      const tabs = s.tabs.filter((t) => t.id !== id);
      if (tabs.length === 0) {
        const fresh: Tab = {
          id: crypto.randomUUID(),
          title: `Untitled ${counter++}`,
          connectionId: null,
          sql: "",
          dirty: false,
        };
        return { tabs: [fresh], activeId: fresh.id };
      }
      const activeId =
        s.activeId === id ? tabs[tabs.length - 1].id : s.activeId;
      return { tabs, activeId };
    });
  },

  updateSql(id, sql) {
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, sql, dirty: true } : t)),
    }));
  },

  setActive(id) {
    set({ activeId: id });
  },

  bindConnection(id, connectionId) {
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, connectionId } : t)),
    }));
  },

  setResult(id, result) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id
          ? { ...t, lastResult: result, lastError: undefined, busy: false }
          : t,
      ),
    }));
  },

  setError(id, message) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id
          ? { ...t, lastError: message, lastResult: undefined, busy: false }
          : t,
      ),
    }));
  },

  setBusy(id, busy) {
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, busy } : t)),
    }));
  },
}));
