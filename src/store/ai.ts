import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";

import type { AiProvider, ProviderConfig } from "@/lib/types";

interface AiState {
  /** Per-provider config. Raw API key is NEVER stored — only apiKeyPresent. */
  providers: Record<AiProvider, ProviderConfig>;
  setProviderConfig: (p: AiProvider, patch: Partial<ProviderConfig>) => void;
  /** Most recent NL prompt (for re-prompt UI). */
  lastPrompt: string;
  setLastPrompt: (s: string) => void;
}

const defaults: Record<AiProvider, ProviderConfig> = {
  openai: {
    provider: "openai",
    apiKeyPresent: false,
    generationModel: "gpt-4o-mini",
    embeddingModel: "text-embedding-3-small",
  },
  anthropic: {
    provider: "anthropic",
    apiKeyPresent: false,
    generationModel: "claude-haiku-4-5",
    embeddingModel: undefined,
  },
  gemini: {
    provider: "gemini",
    apiKeyPresent: false,
    generationModel: "gemini-2.5-flash",
    embeddingModel: "text-embedding-004",
  },
  ollama: {
    provider: "ollama",
    apiKeyPresent: false,
    baseUrl: "http://localhost:11434",
    generationModel: "llama3.1:8b",
    embeddingModel: "nomic-embed-text",
  },
};

export const useAi = create<AiState>()(
  persist(
    (set) => ({
      providers: defaults,
      setProviderConfig: (p, patch) =>
        set((s) => ({
          providers: {
            ...s.providers,
            [p]: { ...s.providers[p], ...patch },
          },
        })),
      lastPrompt: "",
      setLastPrompt: (s) => set({ lastPrompt: s }),
    }),
    {
      name: "tusk-ai",
      storage: createJSONStorage(() => localStorage),
      partialize: (s) => ({
        providers: s.providers,
        lastPrompt: s.lastPrompt,
      }),
    },
  ),
);
