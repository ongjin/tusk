import { create } from "zustand";

interface OpenUmapArgs {
  connId: string;
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  dim: number;
}

interface OpenIndexPanelArgs {
  connId: string;
  schema: string;
  table: string;
}

interface State {
  openUmap: ((a: OpenUmapArgs) => void) | null;
  openIndexPanel: ((a: OpenIndexPanelArgs) => void) | null;
  setOpenUmap: (fn: ((a: OpenUmapArgs) => void) | null) => void;
  setOpenIndexPanel: (fn: ((a: OpenIndexPanelArgs) => void) | null) => void;
}

export const useVectorActions = create<State>((set) => ({
  openUmap: null,
  openIndexPanel: null,
  setOpenUmap: (fn) => set({ openUmap: fn }),
  setOpenIndexPanel: (fn) => set({ openIndexPanel: fn }),
}));
