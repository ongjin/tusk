import { createAnthropic } from "@ai-sdk/anthropic";
import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { createOpenAI } from "@ai-sdk/openai";
import { createOllama } from "ollama-ai-provider-v2";
import type { LanguageModel } from "ai";

import type { AiProvider } from "@/lib/types";

export interface BuildModelArgs {
  provider: AiProvider;
  modelId: string;
  apiKey: string;
  baseUrl?: string;
}

export function buildModel(args: BuildModelArgs): LanguageModel {
  switch (args.provider) {
    case "openai": {
      const oai = createOpenAI({
        apiKey: args.apiKey,
        ...(args.baseUrl ? { baseURL: args.baseUrl } : {}),
      });
      return oai(args.modelId);
    }
    case "anthropic": {
      const anth = createAnthropic({
        apiKey: args.apiKey,
        ...(args.baseUrl ? { baseURL: args.baseUrl } : {}),
      });
      return anth(args.modelId);
    }
    case "gemini": {
      const google = createGoogleGenerativeAI({
        apiKey: args.apiKey,
        ...(args.baseUrl ? { baseURL: args.baseUrl } : {}),
      });
      return google(args.modelId);
    }
    case "ollama": {
      const ollama = createOllama({
        baseURL:
          (args.baseUrl ?? "http://localhost:11434").replace(/\/$/, "") +
          "/api",
      });
      return ollama(args.modelId);
    }
  }
}

/** Suggested defaults for the model picker UI. */
export const DEFAULT_GENERATION_MODELS: Record<AiProvider, string[]> = {
  openai: ["gpt-4o-mini", "gpt-4o", "o4-mini"],
  anthropic: ["claude-haiku-4-5", "claude-sonnet-4-6", "claude-opus-4-7"],
  gemini: ["gemini-2.5-flash", "gemini-2.5-pro"],
  ollama: ["llama3.1:8b", "llama3.2", "qwen2.5:7b"],
};

export const DEFAULT_EMBEDDING_MODELS: Record<AiProvider, string | null> = {
  openai: "text-embedding-3-small",
  anthropic: null,
  gemini: "text-embedding-004",
  ollama: "nomic-embed-text",
};
