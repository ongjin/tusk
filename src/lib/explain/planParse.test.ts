import { describe, expect, it } from "vitest";
import { parsePlan } from "./planParse";
import type { RawExplainPlan } from "./planTypes";

const fixture = {
  analyze: {
    Plan: {
      "Node Type": "Hash Join",
      "Startup Cost": 0,
      "Total Cost": 100,
      "Plan Rows": 10,
      "Plan Width": 64,
      "Actual Startup Time": 0.1,
      "Actual Total Time": 12.0,
      "Actual Rows": 100,
      "Actual Loops": 1,
      Plans: [
        {
          "Node Type": "Seq Scan",
          "Relation Name": "users",
          Schema: "public",
          "Startup Cost": 0,
          "Total Cost": 50,
          "Plan Rows": 50,
          "Plan Width": 32,
          "Actual Startup Time": 0.05,
          "Actual Total Time": 8.0,
          "Actual Rows": 50,
          "Actual Loops": 1,
          Filter: "(email = 'a')",
          "Rows Removed by Filter": 0,
        },
        {
          "Node Type": "Index Scan",
          "Relation Name": "orders",
          Schema: "public",
          "Startup Cost": 0,
          "Total Cost": 30,
          "Plan Rows": 100,
          "Plan Width": 32,
          "Actual Startup Time": 0.05,
          "Actual Total Time": 0.5,
          "Actual Rows": 100,
          "Actual Loops": 1,
        },
      ],
    },
  } satisfies RawExplainPlan,
  planOnly: {
    Plan: {
      "Node Type": "Seq Scan",
      "Relation Name": "users",
      Schema: "public",
      "Startup Cost": 0,
      "Total Cost": 100,
      "Plan Rows": 1000,
      "Plan Width": 32,
    },
  } satisfies RawExplainPlan,
};

describe("parsePlan — analyze", () => {
  it("computes selfMs as parent total minus child totals", () => {
    const root = parsePlan(fixture.analyze);
    expect(root.actualTotalTime).toBe(12);
    expect(root.selfMs).toBeCloseTo(12 - 8 - 0.5, 5);
  });

  it("computes selfTimeRatio against root total", () => {
    const root = parsePlan(fixture.analyze);
    const seqScan = root.children[0];
    expect(seqScan.selfTimeRatio).toBeCloseTo(8 / 12, 5);
  });

  it("populates relationName/schema/filter for leaves", () => {
    const root = parsePlan(fixture.analyze);
    expect(root.children[0]).toMatchObject({
      relationName: "users",
      schema: "public",
      filter: "(email = 'a')",
    });
  });
});

describe("parsePlan — plan-only", () => {
  it("leaves actual fields null and uses selfCostRatio as fallback", () => {
    const root = parsePlan(fixture.planOnly);
    expect(root.actualTotalTime).toBeNull();
    expect(root.selfMs).toBeNull();
    expect(root.selfTimeRatio).toBeNull();
    expect(root.selfCostRatio).toBeCloseTo(1, 5);
  });
});
