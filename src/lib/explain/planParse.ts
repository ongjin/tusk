import type { PlanNode, RawExplainPlan, RawPlanNode } from "./planTypes";

const MAX_DEPTH = 100;

export function parsePlan(raw: RawExplainPlan): PlanNode {
  const root = raw.Plan;
  const rootTotalMs =
    typeof root["Actual Total Time"] === "number"
      ? root["Actual Total Time"]
      : null;
  const rootTotalCost = root["Total Cost"] || 1;
  return walk(root, rootTotalMs, rootTotalCost, 0);
}

function walk(
  raw: RawPlanNode,
  rootTotalMs: number | null,
  rootTotalCost: number,
  depth: number,
): PlanNode {
  if (depth >= MAX_DEPTH) {
    return {
      nodeType: `${raw["Node Type"]} (truncated at depth ${MAX_DEPTH})`,
      startupCost: 0,
      totalCost: 0,
      planRows: 0,
      planWidth: 0,
      actualStartupTime: null,
      actualTotalTime: null,
      actualLoops: null,
      actualRows: null,
      rowsRemovedByFilter: null,
      buffers: null,
      children: [],
      selfMs: null,
      selfTimeRatio: null,
      selfCostRatio: 0,
    };
  }

  const children = (raw.Plans ?? []).map((c) =>
    walk(c, rootTotalMs, rootTotalCost, depth + 1),
  );
  const childTotalMs = children.reduce<number | null>(
    (a, c) =>
      a !== null && c.actualTotalTime !== null ? a + c.actualTotalTime : null,
    0,
  );
  const total =
    typeof raw["Actual Total Time"] === "number"
      ? raw["Actual Total Time"]
      : null;
  const selfMs =
    total !== null && childTotalMs !== null
      ? Math.max(0, total - childTotalMs)
      : null;
  const selfTimeRatio =
    selfMs !== null && rootTotalMs && rootTotalMs > 0
      ? selfMs / rootTotalMs
      : null;

  const childTotalCost = children.reduce((a, c) => a + c.totalCost, 0);
  const selfCost = Math.max(0, raw["Total Cost"] - childTotalCost);
  const selfCostRatio = rootTotalCost > 0 ? selfCost / rootTotalCost : 0;

  const buffersFromContainer = raw.Buffers;
  const buffersFromInline =
    raw["Shared Hit Blocks"] !== undefined ||
    raw["Shared Read Blocks"] !== undefined ||
    raw["Shared Written Blocks"] !== undefined
      ? {
          "Shared Hit Blocks": raw["Shared Hit Blocks"],
          "Shared Read Blocks": raw["Shared Read Blocks"],
          "Shared Written Blocks": raw["Shared Written Blocks"],
        }
      : undefined;
  const buffersRaw = buffersFromContainer ?? buffersFromInline;

  return {
    nodeType: raw["Node Type"],
    relationName: raw["Relation Name"],
    schema: raw.Schema,
    alias: raw.Alias,
    startupCost: raw["Startup Cost"],
    totalCost: raw["Total Cost"],
    planRows: raw["Plan Rows"],
    planWidth: raw["Plan Width"],
    actualStartupTime: raw["Actual Startup Time"] ?? null,
    actualTotalTime: total,
    actualLoops: raw["Actual Loops"] ?? null,
    actualRows: raw["Actual Rows"] ?? null,
    rowsRemovedByFilter: raw["Rows Removed by Filter"] ?? null,
    filter: raw.Filter,
    indexCond: raw["Index Cond"],
    joinType: raw["Join Type"],
    hashCond: raw["Hash Cond"],
    mergeCond: raw["Merge Cond"],
    output: raw.Output,
    buffers: buffersRaw
      ? {
          hit: buffersRaw["Shared Hit Blocks"] ?? 0,
          read: buffersRaw["Shared Read Blocks"] ?? 0,
          written: buffersRaw["Shared Written Blocks"] ?? 0,
        }
      : null,
    children,
    selfMs,
    selfTimeRatio,
    selfCostRatio,
  };
}
