import { create } from "zustand";
import { persist } from "zustand/middleware";

import type { AiProvider } from "@/lib/types";

interface SettingsState {
  // Existing — keep as-is
  autoLimit: number;
  setAutoLimit: (v: number) => void;
  editConflictMode: "pkOnly" | "strict";
  setEditConflictMode: (m: "pkOnly" | "strict") => void;

  // Week 4
  enabledProviders: AiProvider[];
  defaultGenerationProvider: AiProvider;
  defaultEmbeddingProvider: AiProvider;
  toolsEnabled: { sampleRows: boolean };
  destructiveStrict: boolean;
  ragTopK: number;
  schemaIndexAutoSync: boolean;
  setEnabledProviders: (v: AiProvider[]) => void;
  setDefaultGenerationProvider: (p: AiProvider) => void;
  setDefaultEmbeddingProvider: (p: AiProvider) => void;
  setSampleRowsEnabled: (v: boolean) => void;
  setDestructiveStrict: (v: boolean) => void;
  setRagTopK: (v: number) => void;
  setSchemaIndexAutoSync: (v: boolean) => void;
}

export const useSettings = create<SettingsState>()(
  persist(
    (set) => ({
      autoLimit: 1000,
      setAutoLimit: (v) => set({ autoLimit: v }),
      editConflictMode: "pkOnly",
      setEditConflictMode: (m) => set({ editConflictMode: m }),

      enabledProviders: [],
      defaultGenerationProvider: "openai",
      defaultEmbeddingProvider: "openai",
      toolsEnabled: { sampleRows: false },
      destructiveStrict: false,
      ragTopK: 8,
      schemaIndexAutoSync: true,
      setEnabledProviders: (v) => set({ enabledProviders: v }),
      setDefaultGenerationProvider: (p) =>
        set({ defaultGenerationProvider: p }),
      setDefaultEmbeddingProvider: (p) => set({ defaultEmbeddingProvider: p }),
      setSampleRowsEnabled: (v) =>
        set((s) => ({ toolsEnabled: { ...s.toolsEnabled, sampleRows: v } })),
      setDestructiveStrict: (v) => set({ destructiveStrict: v }),
      setRagTopK: (v) => set({ ragTopK: Math.max(1, Math.min(32, v)) }),
      setSchemaIndexAutoSync: (v) => set({ schemaIndexAutoSync: v }),
    }),
    { name: "tusk-settings" },
  ),
);
