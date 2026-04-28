import { useCallback, useEffect, useRef } from "react";
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
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [activeTab.id, closeTab, connectionForTab, newTab, run]);

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
          <Button size="sm" onClick={run} disabled={activeTab.busy}>
            <Play /> Run ({platformModifier() === "meta" ? "⌘" : "Ctrl"}+Enter)
          </Button>
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
          />
          {activeTab.lastResult && connectionForTab && (
            <ResultsGrid
              result={activeTab.lastResult}
              connId={connectionForTab}
            />
          )}
        </div>
      </div>
    </div>
  );
}
