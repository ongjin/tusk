import { create } from "zustand";

import type { ExplainResult, AiInterpretation } from "@/lib/explain/planTypes";
import type { QueryResult } from "@/lib/types";

export interface PlanState {
  result: ExplainResult;
  selectedNodePath: number[];
  aiCacheByKey: Record<string, AiInterpretation>;
  activeAiKey: string | null;
  sqlAtRun: string;
}

export interface Tab {
  id: string;
  title: string;
  connectionId: string | null;
  sql: string;
  dirty: boolean;
  lastResult?: QueryResult;
  lastError?: string;
  busy?: boolean;
  lastPlan?: PlanState;
  resultMode: "rows" | "plan";
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
  setPlan: (id: string, result: ExplainResult, sqlAtRun: string) => void;
  setSelectedNodePath: (id: string, path: number[]) => void;
  setActiveAiKey: (id: string, key: string | null) => void;
  cacheAi: (id: string, key: string, interpretation: AiInterpretation) => void;
  setResultMode: (id: string, mode: "rows" | "plan") => void;
}

let counter = 1;

const initialId = crypto.randomUUID();
const initialTab: Tab = {
  id: initialId,
  title: `Untitled ${counter++}`,
  connectionId: null,
  sql: "SELECT 1",
  dirty: false,
  resultMode: "rows",
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
          resultMode: "rows",
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
          resultMode: "rows",
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
          ? {
              ...t,
              lastResult: result,
              lastError: undefined,
              busy: false,
              resultMode: "rows",
            }
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

  setPlan(id, result, sqlAtRun) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id
          ? {
              ...t,
              lastPlan: {
                result,
                selectedNodePath: [],
                aiCacheByKey: {},
                activeAiKey: null,
                sqlAtRun,
              },
              resultMode: "plan",
              busy: false,
            }
          : t,
      ),
    }));
  },

  setSelectedNodePath(id, path) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id && t.lastPlan
          ? { ...t, lastPlan: { ...t.lastPlan, selectedNodePath: path } }
          : t,
      ),
    }));
  },

  setActiveAiKey(id, key) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id && t.lastPlan
          ? { ...t, lastPlan: { ...t.lastPlan, activeAiKey: key } }
          : t,
      ),
    }));
  },

  cacheAi(id, key, interpretation) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id && t.lastPlan
          ? {
              ...t,
              lastPlan: {
                ...t.lastPlan,
                aiCacheByKey: {
                  ...t.lastPlan.aiCacheByKey,
                  [key]: interpretation,
                },
                activeAiKey: key,
              },
            }
          : t,
      ),
    }));
  },

  setResultMode(id, mode) {
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, resultMode: mode } : t)),
    }));
  },
}));
