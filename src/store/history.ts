import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

import type { HistoryEntry } from "@/lib/types";

interface HistoryState {
  entries: HistoryEntry[];
  search(query: string, connId?: string): Promise<void>;
}

export const useHistory = create<HistoryState>((set) => ({
  entries: [],
  async search(query, connId) {
    const entries = await invoke<HistoryEntry[]>("list_history", {
      connectionId: connId ?? null,
      query: query.trim().length === 0 ? null : query,
      limit: 50,
    });
    set({ entries });
  },
}));
