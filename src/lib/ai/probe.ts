import { generateText } from "ai";

import { aiSecretGet } from "@/lib/keychain";
import { buildModel } from "@/lib/ai/providers";
import type { AiProvider } from "@/lib/types";

interface ProbeArgs {
  provider: AiProvider;
  modelId: string;
  baseUrl?: string;
}

export async function probeProvider(args: ProbeArgs): Promise<{
  ok: boolean;
  message: string;
}> {
  // eslint-disable-next-line no-useless-assignment
  let apiKey: string | null = null;
  try {
    apiKey = await aiSecretGet(args.provider);
  } catch (e) {
    return { ok: false, message: `keychain: ${asMsg(e)}` };
  }
  if (apiKey === null && args.provider !== "ollama") {
    return { ok: false, message: "no key set" };
  }
  try {
    const model = buildModel({
      provider: args.provider,
      modelId: args.modelId,
      apiKey: apiKey ?? "",
      baseUrl: args.baseUrl,
    });
    const r = await generateText({
      model,
      prompt: 'Reply with the single word "pong".',
      maxRetries: 0,
    });
    const text = (r.text ?? "").toLowerCase();
    return text.includes("pong")
      ? { ok: true, message: "pong" }
      : { ok: true, message: `responded: ${text.slice(0, 60)}` };
  } catch (e) {
    return { ok: false, message: asMsg(e) };
  } finally {
    apiKey = null; // eslint-disable-line no-useless-assignment
  }
}

function asMsg(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}
