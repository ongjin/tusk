import { useCallback, useEffect, useRef, useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import { Play } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { executeQuery } from "@/lib/tauri";
import { withAutoLimit } from "@/lib/sql";
import { useTheme } from "@/hooks/use-theme";
import { useConnections } from "@/store/connections";
import { useSettings } from "@/store/settings";
import { useTabs } from "@/store/tabs";
import { ResultsGrid } from "@/features/results/ResultsGrid";
import { ResultsHeader } from "@/features/results/ResultsHeader";
import { ExplainView } from "@/features/explain/ExplainView";
import { runExplainGate } from "@/features/explain/explainGate";

import { runGate } from "@/lib/ai/runGate";
import { invoke } from "@tauri-apps/api/core";

import { CmdKPalette, type ApplyMeta } from "@/features/ai/CmdKPalette";
import { UmapTab } from "@/features/vector/UmapTab";
import { EditorTabs } from "./EditorTabs";
import { isModifier, platformModifier } from "./keymap";

export function EditorPane() {
  const { theme } = useTheme();
  const autoLimit = useSettings((s) => s.autoLimit);
  const tabs = useTabs((s) => s.tabs);
  const activeId = useTabs((s) => s.activeId);
  const updateSql = useTabs((s) => s.updateSql);
  const newTab = useTabs((s) => s.newTab);
  const closeTab = useTabs((s) => s.closeTab);
  const setBusy = useTabs((s) => s.setBusy);
  const setResult = useTabs((s) => s.setResult);
  const setError = useTabs((s) => s.setError);
  const activeConnection = useConnections((s) => s.activeId);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const [showCmdK, setShowCmdK] = useState(false);
  const [selection, setSelection] = useState("");

  const activeTab = tabs.find((t) => t.id === activeId) ?? tabs[0];
  const connectionForTab = activeTab.connectionId ?? activeConnection;
  const connectionName = useConnections(
    (s) =>
      s.items.find((c) => c.id === connectionForTab)?.name ?? connectionForTab,
  );

  const run = useCallback(async () => {
    if (!connectionForTab) {
      toast.error("Select a connected database first");
      return;
    }
    setBusy(activeTab.id, true);
    try {
      const sqlToRun =
        autoLimit > 0 ? withAutoLimit(activeTab.sql, autoLimit) : activeTab.sql;
      const proceed = await runGate(sqlToRun);
      if (!proceed) {
        setBusy(activeTab.id, false);
        return;
      }
      const result = await executeQuery(connectionForTab, sqlToRun);
      setResult(activeTab.id, result);
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Query failed";
      setError(activeTab.id, msg);
      toast.error(msg);
    }
  }, [
    activeTab.id,
    activeTab.sql,
    autoLimit,
    connectionForTab,
    setBusy,
    setError,
    setResult,
  ]);

  const runRequestId = useTabs((s) => s.runRequestId);
  useEffect(() => {
    if (runRequestId > 0) {
      void run();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runRequestId]);

  const runExplainAction = useCallback(
    async (analyzeAnyway = false) => {
      if (!connectionForTab) {
        toast.error("Select a connected database first");
        return;
      }
      setBusy(activeTab.id, true);
      try {
        const r = await runExplainGate({
          connId: connectionForTab,
          sql: activeTab.sql,
          allowAnalyzeAnyway: analyzeAnyway,
        });
        if (r) useTabs.getState().setPlan(activeTab.id, r, activeTab.sql);
        else useTabs.getState().setBusy(activeTab.id, false);
      } catch (e) {
        const msg = e instanceof Error ? e.message : "Explain failed";
        setError(activeTab.id, msg);
        toast.error(msg);
      }
    },
    [activeTab.id, activeTab.sql, connectionForTab, setBusy, setError],
  );

  useEffect(() => {
    const mod = platformModifier();
    function onKey(e: KeyboardEvent) {
      if (!isModifier(e, mod)) return;
      if (e.key === "Enter") {
        e.preventDefault();
        run();
      } else if (e.key.toLowerCase() === "t") {
        e.preventDefault();
        newTab(connectionForTab);
      } else if (e.key.toLowerCase() === "w") {
        e.preventDefault();
        closeTab(activeTab.id);
      } else if (e.key.toLowerCase() === "k") {
        e.preventDefault();
        const ed = editorRef.current;
        let sel = "";
        if (ed) {
          const m = ed.getModel();
          const r = ed.getSelection();
          if (m && r) sel = m.getValueInRange(r);
        }
        setSelection(sel);
        setShowCmdK(true);
      } else if (e.key.toLowerCase() === "e" && e.shiftKey) {
        e.preventDefault();
        runExplainAction(false);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [activeTab.id, closeTab, connectionForTab, newTab, run, runExplainAction]);

  const handleCmdKApply = useCallback(
    async (sql: string, meta: ApplyMeta) => {
      const ed = editorRef.current;
      if (ed) {
        const m = ed.getModel();
        const r = ed.getSelection();
        if (m && r) {
          ed.executeEdits("cmdk-apply", [
            { range: r, text: sql, forceMoveMarkers: true },
          ]);
        } else {
          const next =
            activeTab.sql + (activeTab.sql.endsWith("\n") ? "" : "\n") + sql;
          updateSql(activeTab.id, next);
        }
      }
      if (connectionForTab) {
        try {
          await invoke("record_ai_generation", {
            payload: {
              connId: connectionForTab,
              prompt: meta.prompt,
              generatedSql: sql,
              provider: meta.provider,
              generationModel: meta.generationModel,
              embeddingModel: meta.embeddingModel ?? null,
              topKTables: meta.topKTables,
              toolCalls: meta.toolCalls,
              promptTokens: meta.promptTokens ?? null,
              completionTokens: meta.completionTokens ?? null,
              durationMs: 0,
            },
          });
        } catch (e) {
          toast.error(
            `Failed to record AI history: ${e instanceof Error ? e.message : e}`,
          );
        }
      }
      setShowCmdK(false);
    },
    [activeTab.id, activeTab.sql, connectionForTab, updateSql],
  );

  if (activeTab.umap) {
    return <UmapTab tabId={activeTab.id} />;
  }

  return (
    <div className="flex flex-1 flex-col">
      <EditorTabs />
      <div className="flex flex-1 flex-col">
        <div className="border-border flex items-center justify-between border-b px-3 py-1.5">
          <span className="text-muted-foreground text-xs">
            {connectionForTab
              ? `Running on: ${connectionName}`
              : "No connection"}
          </span>
          <div className="flex items-center gap-2">
            <Button size="sm" onClick={run} disabled={activeTab.busy}>
              <Play /> Run ({platformModifier() === "meta" ? "⌘" : "Ctrl"}
              +Enter)
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => runExplainAction(false)}
              disabled={activeTab.busy}
            >
              Explain ({platformModifier() === "meta" ? "⌘⇧" : "Ctrl+Shift+"}E)
            </Button>
          </div>
        </div>
        <div className="min-h-0 flex-1">
          <Editor
            height="100%"
            language="sql"
            theme={theme === "dark" ? "vs-dark" : "vs"}
            value={activeTab.sql}
            onChange={(v) => updateSql(activeTab.id, v ?? "")}
            onMount={(ed) => {
              editorRef.current = ed;
            }}
            options={{
              minimap: { enabled: false },
              fontSize: 13,
              fontFamily:
                "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
              scrollBeyondLastLine: false,
              automaticLayout: true,
              wordWrap: "on",
            }}
          />
        </div>
        <div className="flex max-h-[45vh] min-h-[120px] flex-col">
          <ResultsHeader
            result={activeTab.lastResult}
            error={activeTab.lastError}
            busy={activeTab.busy}
            connId={connectionForTab}
            hasPlan={!!activeTab.lastPlan}
            resultMode={activeTab.resultMode}
            onModeChange={(mode) =>
              useTabs.getState().setResultMode(activeTab.id, mode)
            }
          />
          {activeTab.resultMode === "plan" &&
          activeTab.lastPlan &&
          connectionForTab ? (
            <ExplainView
              tabId={activeTab.id}
              connId={connectionForTab}
              sql={activeTab.sql}
              result={activeTab.lastPlan.result}
            />
          ) : (
            activeTab.lastResult &&
            connectionForTab && (
              <ResultsGrid
                result={activeTab.lastResult}
                connId={connectionForTab}
              />
            )
          )}
        </div>
      </div>
      <CmdKPalette
        open={showCmdK}
        connectionId={connectionForTab ?? undefined}
        selection={selection}
        onClose={() => setShowCmdK(false)}
        onApply={handleCmdKApply}
      />
    </div>
  );
}
