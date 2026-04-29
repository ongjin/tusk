import { useEffect, useMemo, useRef } from "react";

import type { PlanNode } from "@/lib/explain/planTypes";

interface Props {
  root: PlanNode;
  selectedPath: number[];
  onSelect: (path: number[]) => void;
  planOnly: boolean;
}

interface FlatRow {
  node: PlanNode;
  depth: number;
  path: number[];
}

export function PlanTree({ root, selectedPath, onSelect, planOnly }: Props) {
  const rows = useMemo(() => flatten(root), [root]);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!containerRef.current?.contains(document.activeElement)) return;
      const idx = rows.findIndex((r) => samePath(r.path, selectedPath));
      if (e.key === "ArrowDown") {
        e.preventDefault();
        const next = rows[Math.min(rows.length - 1, idx + 1)] ?? rows[0];
        onSelect(next.path);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        const next = rows[Math.max(0, idx - 1)] ?? rows[0];
        onSelect(next.path);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [rows, selectedPath, onSelect]);

  return (
    <div
      ref={containerRef}
      tabIndex={0}
      className="font-mono text-xs outline-none"
      role="tree"
      aria-label="Explain plan tree"
    >
      {rows.map((r, i) => (
        <Row
          key={i}
          row={r}
          selected={samePath(r.path, selectedPath)}
          onSelect={onSelect}
          planOnly={planOnly}
        />
      ))}
    </div>
  );
}

function Row({
  row,
  selected,
  onSelect,
  planOnly,
}: {
  row: FlatRow;
  selected: boolean;
  onSelect: (path: number[]) => void;
  planOnly: boolean;
}) {
  const ratio = planOnly
    ? row.node.selfCostRatio
    : (row.node.selfTimeRatio ?? row.node.selfCostRatio);
  const heavy = ratio >= 0.3;
  const widthPct = Math.min(100, Math.max(0, ratio * 100));
  return (
    <button
      type="button"
      onClick={() => onSelect(row.path)}
      className={`relative flex w-full items-center gap-2 px-2 py-1 text-left ${
        selected ? "bg-accent/40" : "hover:bg-accent/20"
      } ${heavy ? "border-l-2 border-l-red-500" : "border-l-2 border-l-transparent"}`}
      role="treeitem"
      aria-selected={selected}
    >
      <span
        aria-hidden
        className="absolute inset-y-0 left-0 bg-amber-500/30"
        style={{ width: `${widthPct}%` }}
      />
      <span className="relative flex-1" style={{ paddingLeft: row.depth * 14 }}>
        <span className="text-muted-foreground">▸</span>{" "}
        <span className="font-medium">{row.node.nodeType}</span>
        {row.node.relationName && (
          <span className="text-muted-foreground">
            {" "}
            · {row.node.schema ?? "public"}.{row.node.relationName}
          </span>
        )}
        {!planOnly && row.node.selfMs !== null && (
          <span className="text-muted-foreground">
            {" "}
            · {row.node.selfMs.toFixed(1)} ms
          </span>
        )}
        <span className="text-muted-foreground">
          {" · "}
          {row.node.actualRows ?? row.node.planRows}{" "}
          {planOnly ? "est rows" : "rows"}
        </span>
        {heavy && <span className="text-red-500"> ⚠</span>}
      </span>
    </button>
  );
}

function flatten(node: PlanNode, depth = 0, path: number[] = []): FlatRow[] {
  const out: FlatRow[] = [{ node, depth, path }];
  node.children.forEach((c, i) => {
    out.push(...flatten(c, depth + 1, [...path, i]));
  });
  return out;
}

function samePath(a: number[], b: number[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}
