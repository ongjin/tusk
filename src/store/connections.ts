import { create } from "zustand";

import {
  addConnection as addConnectionCmd,
  connect as connectCmd,
  deleteConnection as deleteConnectionCmd,
  disconnect as disconnectCmd,
  listConnections,
} from "@/lib/tauri";
import type { ConnectionListItem, NewConnection } from "@/lib/types";
import { useSchema } from "@/store/schema";

interface ConnectionsState {
  items: ConnectionListItem[];
  activeId: string | null;
  loading: boolean;
  refresh: () => Promise<void>;
  add: (newConnection: NewConnection, password: string) => Promise<void>;
  remove: (id: string) => Promise<void>;
  connect: (id: string) => Promise<void>;
  disconnect: (id: string) => Promise<void>;
  setActive: (id: string | null) => void;
}

export const useConnections = create<ConnectionsState>((set, get) => ({
  items: [],
  activeId: null,
  loading: false,

  async refresh() {
    set({ loading: true });
    try {
      const items = await listConnections();
      set({ items });
    } finally {
      set({ loading: false });
    }
  },

  async add(newConnection, password) {
    await addConnectionCmd(newConnection, password);
    await get().refresh();
  },

  async remove(id) {
    await deleteConnectionCmd(id);
    useSchema.getState().clear(id);
    if (get().activeId === id) set({ activeId: null });
    await get().refresh();
  },

  async connect(id) {
    await connectCmd(id);
    set({ activeId: id });
    await get().refresh();
  },

  async disconnect(id) {
    await disconnectCmd(id);
    useSchema.getState().clear(id);
    if (get().activeId === id) set({ activeId: null });
    await get().refresh();
  },

  setActive(id) {
    set({ activeId: id });
  },
}));
