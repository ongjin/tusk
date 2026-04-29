import type { ExplainResult } from "@/lib/explain/planTypes";

interface Props {
  result: ExplainResult;
}

export function ExplainView({ result }: Props) {
  return (
    <div className="flex h-full flex-col">
      <header className="border-border bg-muted/30 flex items-center gap-2 border-b px-3 py-1.5 text-xs">
        <span className="rounded bg-amber-500/20 px-2 py-0.5">
          {result.mode}
        </span>
        {result.totalMs !== null && (
          <span className="text-muted-foreground">
            {result.totalMs.toFixed(1)} ms
          </span>
        )}
        {result.warnings.length > 0 && (
          <span className="text-amber-600" title={result.warnings.join("\n")}>
            ⚠ {result.warnings.length} warning(s)
          </span>
        )}
      </header>
      <div className="flex-1 overflow-auto p-3 font-mono text-xs">
        Plan tree placeholder — node count: {countNodes(result.plan)}
      </div>
    </div>
  );
}

function countNodes(node: { children: unknown[] }): number {
  return (
    1 +
    (node.children as { children: unknown[] }[]).reduce(
      (a, c) => a + countNodes(c as never),
      0,
    )
  );
}
