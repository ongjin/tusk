import type { ExplainResult, PlanNode } from "@/lib/explain/planTypes";
import { useTabs } from "@/store/tabs";

import { AnalyzeAnywayButton } from "./AnalyzeAnywayButton";
import { IndexCandidates } from "./IndexCandidates";
import { PlanAiStrip } from "./PlanAiStrip";
import { PlanNodeDetail } from "./PlanNodeDetail";
import { PlanTree } from "./PlanTree";

interface Props {
  tabId: string;
  connId: string;
  sql: string;
  result: ExplainResult;
}

export function ExplainView({ tabId, connId, sql, result }: Props) {
  const tab = useTabs((s) => s.tabs.find((t) => t.id === tabId));
  const selectedPath = tab?.lastPlan?.selectedNodePath ?? [];
  const setSelectedNodePath = useTabs((s) => s.setSelectedNodePath);
  const planOnly =
    result.mode === "dml-plan-only" || result.mode === "ddl-plan-only";
  const stale = tab?.sql !== tab?.lastPlan?.sqlAtRun;

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
        {stale && (
          <span className="rounded bg-orange-500/20 px-2 py-0.5">
            stale (sql edited)
          </span>
        )}
        {(result.mode === "dml-plan-only" ||
          result.mode === "ddl-plan-only") && (
          <>
            <span className="rounded bg-yellow-500/20 px-2 py-0.5 text-yellow-700">
              Estimated only — would modify data
            </span>
            <AnalyzeAnywayButton tabId={tabId} connId={connId} sql={sql} />
          </>
        )}
        {result.mode === "analyze-anyway-rolled-back" && (
          <span className="rounded bg-green-500/20 px-2 py-0.5">
            ANALYZE (rolled back)
          </span>
        )}
        {result.mode === "analyze-anyway-in-tx" && (
          <span className="rounded bg-amber-500/20 px-2 py-0.5">
            ANALYZE (in active tx)
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
        <div className="overflow-auto">
          <PlanNodeDetail
            node={selectedNode(result.plan, selectedPath)}
            planOnly={planOnly}
          />
        </div>
      </div>
      <IndexCandidates
        candidates={result.verifiedCandidates}
        onInsert={(sqlText) => {
          const t = useTabs.getState();
          const found = t.tabs.find((x) => x.id === tabId);
          if (!found) return;
          const next =
            found.sql + (found.sql.endsWith("\n") ? "" : "\n") + sqlText + "\n";
          t.updateSql(tabId, next);
        }}
      />
      <PlanAiStrip tabId={tabId} connId={connId} result={result} sql={sql} />
    </div>
  );
}

function selectedNode(root: PlanNode, path: number[]): PlanNode | null {
  let cur: PlanNode | undefined = root;
  for (const idx of path) {
    cur = cur?.children[idx];
    if (!cur) return null;
  }
  return cur ?? null;
}
