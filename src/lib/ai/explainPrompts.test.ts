import { describe, expect, it } from "vitest";

import { buildExplainUserPrompt, compactPlanText } from "./explainPrompts";
import type { ExplainResult, PlanNode } from "@/lib/explain/planTypes";

const seqScan: PlanNode = {
  nodeType: "Seq Scan",
  relationName: "users",
  schema: "public",
  startupCost: 0,
  totalCost: 50,
  planRows: 50,
  planWidth: 32,
  actualStartupTime: 0,
  actualTotalTime: 8,
  actualLoops: 1,
  actualRows: 50,
  rowsRemovedByFilter: 0,
  filter: "(email = 'a')",
  buffers: null,
  children: [],
  selfMs: 8,
  selfTimeRatio: 1,
  selfCostRatio: 1,
};

const result: ExplainResult = {
  mode: "select-analyze",
  planJson: { Plan: { "Node Type": "Seq Scan" } as never },
  plan: seqScan,
  warnings: [],
  verifiedCandidates: [
    {
      schema: "public",
      table: "users",
      columns: ["email"],
      reason: "rows-removed-by-filter",
      verdict: "likely",
      selectivityEstimate: 0.001,
      nDistinct: -1,
      nullFrac: 0,
    },
  ],
  totalMs: 8,
  executedAt: 0,
};

describe("compactPlanText", () => {
  it("indents children", () => {
    const text = compactPlanText(seqScan);
    expect(text).toContain("Seq Scan");
    expect(text).toContain("users");
  });
});

describe("buildExplainUserPrompt", () => {
  it("includes verified candidates JSON and original SQL", () => {
    const out = buildExplainUserPrompt({
      result,
      sql: "SELECT * FROM users WHERE email='a'",
      relations: [
        {
          schema: "public",
          table: "users",
          ddl: "CREATE TABLE public.users (id int, email text)",
          indexes: ["users_pkey ON id"],
          stats: { rowEstimate: 50000 },
        },
      ],
      tokenBudget: 8000,
    });
    expect(out).toContain("Verified candidates");
    expect(out).toContain("CREATE TABLE public.users");
    expect(out).toContain("SELECT * FROM users WHERE email='a'");
  });

  it("drops DDL bodies when over token budget", () => {
    const giantDdl = "x".repeat(40_000);
    const out = buildExplainUserPrompt({
      result,
      sql: "SELECT 1",
      relations: [
        {
          schema: "public",
          table: "users",
          ddl: giantDdl,
          indexes: [],
          stats: { rowEstimate: 50000 },
        },
      ],
      tokenBudget: 1000,
    });
    expect(out).not.toContain(giantDdl);
    expect(out).toContain("(DDL omitted: token budget)");
  });
});
