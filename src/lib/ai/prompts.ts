import type { TopKTable } from "@/lib/ai/types";

export interface BuildSystemPromptArgs {
  pgVersion: string;
  extensions: string[];
  topK: TopKTable[];
  recentSuccessful: string[];
  selectionContext?: string;
}

export function buildSystemPrompt(args: BuildSystemPromptArgs): string {
  const ddlBlock = args.topK
    .map(
      (t) =>
        `-- ${t.schema}.${t.table}${t.forced ? " (forced include)" : ` (sim=${t.similarity.toFixed(2)})`}\n${t.ddl}`,
    )
    .join("\n\n");
  const fewShot =
    args.recentSuccessful.length > 0
      ? `Recent successful queries on this connection:\n${args.recentSuccessful
          .map((s) => `-- recent\n${s}`)
          .join("\n\n")}`
      : "";
  const selection = args.selectionContext
    ? `The user has selected this SQL — produce an edited version that addresses their request while preserving intent:\n\`\`\`sql\n${args.selectionContext}\n\`\`\``
    : "";
  return [
    `You are an expert PostgreSQL author. Target Postgres ${args.pgVersion}.`,
    `Active extensions: ${args.extensions.join(", ") || "(none reported)"}.`,
    "Output rules:",
    "- Reply with a single SQL block in ```sql fences and nothing else.",
    "- Use existing tables and columns from the supplied schema. Do NOT invent identifiers.",
    "- Prefer schema-qualified names (schema.table).",
    "- Destructive operations (DROP/TRUNCATE/DELETE without WHERE/UPDATE without WHERE) require the user to confirm — write them only when the user clearly asks.",
    "- Comment briefly above the SQL when an assumption was needed.",
    "Schema (top-K):",
    ddlBlock,
    fewShot,
    selection,
  ]
    .filter(Boolean)
    .join("\n\n");
}

/** Strip a single ```sql fenced block (or first ``` block) from model text. */
export function extractSql(text: string): string {
  const fenced = /```(?:sql)?\s*([\s\S]+?)```/m.exec(text);
  return fenced?.[1]?.trim() ?? text.trim();
}
