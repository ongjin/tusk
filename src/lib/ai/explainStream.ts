import { streamText, type LanguageModel } from "ai";

import type {
  AiInterpretation,
  AiIndexRecommendation,
} from "@/lib/explain/planTypes";

export interface StreamExplainArgs {
  model: LanguageModel;
  systemPrompt: string;
  userPrompt: string;
  signal?: AbortSignal;
  onChunk?: (text: string) => void;
}

export async function streamExplainInterpretation(
  args: StreamExplainArgs,
): Promise<AiInterpretation> {
  const started = performance.now();
  let buf = "";
  const r = streamText({
    model: args.model,
    system: args.systemPrompt,
    prompt: args.userPrompt,
    abortSignal: args.signal,
  });
  for await (const chunk of r.textStream) {
    buf += chunk;
    args.onChunk?.(buf);
  }
  let promptTokens: number | undefined;
  let completionTokens: number | undefined;
  try {
    const usage = await r.usage;
    promptTokens = usage?.inputTokens ?? undefined;
    completionTokens = usage?.outputTokens ?? undefined;
  } catch {
    /* usage may not always be present */
  }
  return {
    summary: extractFenced(buf, "summary") ?? buf.trim(),
    recommendations: parseRecommendations(extractFenced(buf, "json") ?? "[]"),
    promptTokens,
    completionTokens,
    durationMs: Math.round(performance.now() - started),
  };
}

function extractFenced(text: string, tag: string): string | null {
  const re = new RegExp("```" + tag + "\\s*([\\s\\S]+?)```", "m");
  const m = re.exec(text);
  return m?.[1]?.trim() ?? null;
}

function parseRecommendations(json: string): AiIndexRecommendation[] {
  try {
    const arr = JSON.parse(json);
    if (!Array.isArray(arr)) return [];
    return arr
      .filter(
        (x): x is AiIndexRecommendation =>
          typeof x === "object" &&
          x !== null &&
          typeof x.schema === "string" &&
          typeof x.table === "string" &&
          Array.isArray(x.columns),
      )
      .map((x) => ({
        schema: x.schema,
        table: x.table,
        columns: x.columns.filter((c): c is string => typeof c === "string"),
        type: (x.type as AiIndexRecommendation["type"]) ?? "btree",
        where: typeof x.where === "string" ? x.where : undefined,
        reason: typeof x.reason === "string" ? x.reason : "",
        priority:
          x.priority === "high" ||
          x.priority === "medium" ||
          x.priority === "low"
            ? x.priority
            : "medium",
      }));
  } catch {
    return [];
  }
}
