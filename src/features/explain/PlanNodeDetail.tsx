import type { ReactNode } from "react";

import type { PlanNode } from "@/lib/explain/planTypes";

interface Props {
  node: PlanNode | null;
  planOnly: boolean;
}

function row(label: string, value: ReactNode | null | undefined) {
  if (value === null || value === undefined || value === "") return null;
  return (
    <div className="grid grid-cols-[120px_1fr] gap-2 py-0.5">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-mono">{value}</span>
    </div>
  );
}

export function PlanNodeDetail({ node, planOnly }: Props) {
  if (!node) {
    return (
      <div className="text-muted-foreground p-3 text-xs">
        Click a node to inspect.
      </div>
    );
  }

  const rowsRow = `${node.actualRows ?? "?"} actual / ${node.planRows} estimated`;
  const buffersRow = node.buffers
    ? `hit=${node.buffers.hit} read=${node.buffers.read} written=${node.buffers.written}`
    : null;

  return (
    <div className="p-3 text-xs">
      <h4 className="mb-2 text-sm font-semibold">
        {node.nodeType}
        {node.joinType && (
          <span className="text-muted-foreground"> · {node.joinType}</span>
        )}
      </h4>
      {row(
        "Relation",
        node.relationName
          ? `${node.schema ?? "public"}.${node.relationName}`
          : null,
      )}
      {row("Alias", node.alias)}
      {row("Filter", node.filter)}
      {row("Index Cond", node.indexCond)}
      {row("Hash Cond", node.hashCond)}
      {row("Merge Cond", node.mergeCond)}
      {row("Rows", rowsRow)}
      {!planOnly &&
        row(
          "Time",
          node.actualTotalTime !== null
            ? `total ${node.actualTotalTime.toFixed(2)} ms · self ${(node.selfMs ?? 0).toFixed(2)} ms`
            : null,
        )}
      {!planOnly && row("Loops", node.actualLoops)}
      {row(
        "Cost",
        `startup ${node.startupCost.toFixed(2)} · total ${node.totalCost.toFixed(2)}`,
      )}
      {row("Buffers", buffersRow)}
      {row(
        "Output",
        node.output && node.output.length > 0
          ? node.output.slice(0, 8).join(", ")
          : null,
      )}
    </div>
  );
}
