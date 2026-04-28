import { create } from "zustand";
import { persist } from "zustand/middleware";

interface SettingsState {
  autoLimit: number; // 0 = off
  setAutoLimit: (v: number) => void;
}

export const useSettings = create<SettingsState>()(
  persist(
    (set) => ({
      autoLimit: 1000,
      setAutoLimit: (v) => set({ autoLimit: v }),
    }),
    { name: "tusk-settings" },
  ),
);
