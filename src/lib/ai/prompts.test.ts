import { describe, expect, it } from "vitest";

import { buildSystemPrompt, extractSql } from "./prompts";

describe("buildSystemPrompt", () => {
  it("emits stable structure", () => {
    const out = buildSystemPrompt({
      pgVersion: "16.2",
      extensions: ["pgvector"],
      topK: [
        {
          schema: "public",
          table: "users",
          ddl: "CREATE TABLE public.users (id int);",
          similarity: 0.91,
          forced: false,
        },
      ],
      recentSuccessful: ["SELECT 1"],
    });
    expect(out).toMatch(/Postgres 16.2/);
    expect(out).toMatch(/pgvector/);
    expect(out).toMatch(/public\.users/);
    expect(out).toMatch(/SELECT 1/);
  });

  it("respects selection context", () => {
    const out = buildSystemPrompt({
      pgVersion: "16",
      extensions: [],
      topK: [],
      recentSuccessful: [],
      selectionContext: "SELECT old FROM t",
    });
    expect(out).toMatch(/SELECT old FROM t/);
  });
});

describe("extractSql", () => {
  it("pulls a fenced sql block", () => {
    expect(extractSql("```sql\nSELECT 1\n```")).toBe("SELECT 1");
  });
  it("falls back to trimmed text", () => {
    expect(extractSql("SELECT 1\n")).toBe("SELECT 1");
  });
});
