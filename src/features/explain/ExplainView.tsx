import type { ExplainResult } from "@/lib/explain/planTypes";
import { useTabs } from "@/store/tabs";

import { PlanTree } from "./PlanTree";

interface Props {
  tabId: string;
  result: ExplainResult;
}

export function ExplainView({ tabId, result }: Props) {
  const tab = useTabs((s) => s.tabs.find((t) => t.id === tabId));
  const selectedPath = tab?.lastPlan?.selectedNodePath ?? [];
  const setSelectedNodePath = useTabs((s) => s.setSelectedNodePath);
  const planOnly =
    result.mode === "dml-plan-only" || result.mode === "ddl-plan-only";

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
      <div className="grid flex-1 grid-cols-[1.5fr_1fr] overflow-hidden">
        <div className="overflow-auto border-r">
          <PlanTree
            root={result.plan}
            selectedPath={selectedPath}
            onSelect={(p) => setSelectedNodePath(tabId, p)}
            planOnly={planOnly}
          />
        </div>
        <div className="text-muted-foreground overflow-auto p-3 text-xs">
          Node detail — coming up in Task 14.
        </div>
      </div>
    </div>
  );
}
