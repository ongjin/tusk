import { streamText, type LanguageModel } from "ai";

import { buildTools } from "@/lib/ai/tools";

export interface StreamGenerationArgs {
  model: LanguageModel;
  systemPrompt: string;
  userPrompt: string;
  connectionId: string;
  sampleRowsEnabled: boolean;
  signal: AbortSignal;
  onChunk: (text: string) => void;
}

export interface StreamGenerationResult {
  text: string;
  toolCalls: { name: string; args: unknown }[];
  promptTokens?: number;
  completionTokens?: number;
}

export async function streamGeneration(
  args: StreamGenerationArgs,
): Promise<StreamGenerationResult> {
  const tools = buildTools({
    connectionId: args.connectionId,
    sampleRowsEnabled: args.sampleRowsEnabled,
  });
  const result = streamText({
    model: args.model,
    system: args.systemPrompt,
    prompt: args.userPrompt,
    tools,
    abortSignal: args.signal,
    maxRetries: 1,
  });

  let buf = "";
  for await (const delta of result.textStream) {
    buf += delta;
    args.onChunk(buf);
  }

  const finalCalls: { name: string; args: unknown }[] = [];
  try {
    const calls = await result.toolCalls;
    for (const c of calls) {
      finalCalls.push({ name: c.toolName, args: c.input });
    }
  } catch {
    /* not all SDK responses expose toolCalls; treat as none */
  }

  let promptTokens: number | undefined;
  let completionTokens: number | undefined;
  try {
    const usage = await result.usage;
    promptTokens = usage?.inputTokens ?? undefined;
    completionTokens = usage?.outputTokens ?? undefined;
  } catch {
    /* usage may not exist on all providers */
  }

  return {
    text: buf,
    toolCalls: finalCalls,
    promptTokens,
    completionTokens,
  };
}
