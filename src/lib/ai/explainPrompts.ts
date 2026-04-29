import type { ExplainResult, PlanNode } from "@/lib/explain/planTypes";

export const SYSTEM_EXPLAIN_PROMPT = `You are a Postgres performance reviewer. You receive an EXPLAIN plan plus relation context and produce two artefacts:

1. A single-paragraph plain-English summary identifying the dominant bottleneck. Mention specific node types and durations. Do not narrate the whole tree.
2. A JSON array of index recommendations. ONLY recommend an index if it is likely to help the supplied plan. Do not invent statistics. Prefer composites only when the plan shows multiple correlated filter columns. Skip recommendations whose selectivity is clearly poor (the user has already filtered low-cardinality candidates server-side).

Output exactly two fenced blocks in this order:

\`\`\`summary
<one paragraph>
\`\`\`

\`\`\`json
[
  {
    "schema": "...", "table": "...", "columns": ["..."],
    "type": "btree" | "composite" | "partial",
    "where": "<partial predicate, optional>",
    "reason": "<short>",
    "priority": "high" | "medium" | "low"
  }
]
\`\`\`

If you have nothing useful to recommend, output an empty array \`[]\` in the json block.`;

export interface RelationContext {
  schema: string;
  table: string;
  ddl: string;
  indexes: string[];
  stats: { rowEstimate?: number };
}

export interface BuildExplainPromptArgs {
  result: ExplainResult;
  sql: string;
  relations: RelationContext[];
  tokenBudget: number;
}

export function buildExplainUserPrompt(args: BuildExplainPromptArgs): string {
  const { result, sql, relations } = args;
  const planText = compactPlanText(result.plan);
  const candidates = JSON.stringify(result.verifiedCandidates, null, 2);

  let relationsBlock = relations.map((r) => relationFull(r)).join("\n\n");
  let prompt = compose({ planText, relationsBlock, candidates, sql });

  if (estimateTokens(prompt) > args.tokenBudget) {
    relationsBlock = relations.map((r) => relationStatsOnly(r)).join("\n\n");
    prompt = compose({ planText, relationsBlock, candidates, sql });
  }
  return prompt;
}

function compose({
  planText,
  relationsBlock,
  candidates,
  sql,
}: {
  planText: string;
  relationsBlock: string;
  candidates: string;
  sql: string;
}): string {
  return [
    "Plan (compact tree):",
    planText,
    "",
    "Relations involved:",
    relationsBlock,
    "",
    "Verified candidates (server-side cardinality-filtered):",
    candidates,
    "",
    "Original SQL:",
    sql,
  ].join("\n");
}

function relationFull(r: RelationContext): string {
  return [
    `-- ${r.schema}.${r.table}`,
    r.ddl,
    "Indexes:",
    r.indexes.length === 0
      ? "  (none)"
      : r.indexes.map((i) => `  - ${i}`).join("\n"),
    `Stats: rows≈${r.stats.rowEstimate ?? "?"}`,
  ].join("\n");
}

function relationStatsOnly(r: RelationContext): string {
  return [
    `-- ${r.schema}.${r.table}`,
    "(DDL omitted: token budget)",
    "Indexes:",
    r.indexes.length === 0
      ? "  (none)"
      : r.indexes.map((i) => `  - ${i}`).join("\n"),
    `Stats: rows≈${r.stats.rowEstimate ?? "?"}`,
  ].join("\n");
}

function estimateTokens(s: string): number {
  return Math.ceil(s.length / 4);
}

export function compactPlanText(node: PlanNode, depth = 0): string {
  const indent = "  ".repeat(depth);
  const rel = node.relationName
    ? ` ${node.schema ?? "public"}.${node.relationName}`
    : "";
  const ms =
    node.actualTotalTime !== null
      ? ` ${node.actualTotalTime.toFixed(2)}ms`
      : ` cost=${node.totalCost.toFixed(0)}`;
  const rows = ` rows=${node.actualRows ?? node.planRows}`;
  const filter = node.filter
    ? ` filter=${node.filter}`
    : node.indexCond
      ? ` cond=${node.indexCond}`
      : "";
  const head = `${indent}${node.nodeType}${rel}${ms}${rows}${filter}`;
  const kids = node.children
    .map((c) => compactPlanText(c, depth + 1))
    .join("\n");
  return kids ? `${head}\n${kids}` : head;
}
