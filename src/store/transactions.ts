import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

import type { TxState } from "@/lib/types";

interface Store {
  byConn: Record<string, TxState>;
  begin(connId: string): Promise<void>;
  commit(connId: string): Promise<void>;
  rollback(connId: string): Promise<void>;
  applySnapshot(snap: TxState): void;
}

export const useTransactions = create<Store>((set, get) => ({
  byConn: {},
  applySnapshot(snap) {
    set((s) => ({ byConn: { ...s.byConn, [snap.connId]: snap } }));
  },
  async begin(connId) {
    const snap = await invoke<TxState>("tx_begin", { connectionId: connId });
    get().applySnapshot(snap);
  },
  async commit(connId) {
    const snap = await invoke<TxState>("tx_commit", { connectionId: connId });
    get().applySnapshot({ ...snap, active: false });
  },
  async rollback(connId) {
    const snap = await invoke<TxState>("tx_rollback", { connectionId: connId });
    get().applySnapshot({ ...snap, active: false });
  },
}));

export function isTxActive(connId: string): boolean {
  return useTransactions.getState().byConn[connId]?.active === true;
}
