import { toast } from "sonner";

import { runGate } from "@/lib/ai/runGate";
import { runExplain } from "@/lib/tauri";
import type { ExplainResult } from "@/lib/explain/planTypes";

export async function runExplainGate(args: {
  connId: string;
  sql: string;
  allowAnalyzeAnyway?: boolean;
}): Promise<ExplainResult | null> {
  if (!args.sql.trim()) {
    toast.error("SQL is empty");
    return null;
  }
  if (args.allowAnalyzeAnyway) {
    const proceed = await runGate(args.sql);
    if (!proceed) return null;
  }
  try {
    return await runExplain({
      connectionId: args.connId,
      sql: args.sql,
      allowAnalyzeAnyway: args.allowAnalyzeAnyway,
    });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    toast.error(`Explain failed: ${msg}`);
    return null;
  }
}
