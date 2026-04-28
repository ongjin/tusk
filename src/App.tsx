import { useEffect, useMemo } from "react";
import { Moon, Sun } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toast } from "sonner";

import { ConnectionForm } from "@/features/connections/ConnectionForm";
import { ConnectionList } from "@/features/connections/ConnectionList";
import { SchemaTree } from "@/features/schema/SchemaTree";
import { EditorPane } from "@/features/editor/EditorPane";
import { AutoCommitToggle } from "@/features/transactions/AutoCommitToggle";
import { TxIndicator } from "@/features/transactions/TxIndicator";
import { TxSidePanel } from "@/features/transactions/TxSidePanel";
import { Button } from "@/components/ui/button";
import { ConfirmModalHost, openConfirmModal } from "@/lib/confirm";
import { useTheme } from "@/hooks/use-theme";
import { useConnections } from "@/store/connections";
import { useSettings } from "@/store/settings";
import { useTabs } from "@/store/tabs";
import { useTransactions } from "@/store/transactions";

function App() {
  const { theme, toggle } = useTheme();
  const autoLimit = useSettings((s) => s.autoLimit);
  const setAutoLimit = useSettings((s) => s.setAutoLimit);
  const tabs = useTabs((s) => s.tabs);
  const activeTabId = useTabs((s) => s.activeId);
  const activeConnectionId = useConnections((s) => s.activeId);

  const activeConnId = useMemo(() => {
    const tab = tabs.find((t) => t.id === activeTabId);
    return tab?.connectionId ?? activeConnectionId ?? undefined;
  }, [tabs, activeTabId, activeConnectionId]);

  // Global keyboard shortcuts for tx commit / rollback.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const isMod = e.metaKey || e.ctrlKey;
      if (!isMod || !e.shiftKey) return;
      const connId = activeConnId;
      if (!connId) return;
      const tx = useTransactions.getState().byConn[connId];
      if (!tx?.active) return;
      const key = e.key.toLowerCase();
      if (key === "c") {
        e.preventDefault();
        void useTransactions.getState().commit(connId);
      } else if (key === "r") {
        e.preventDefault();
        void useTransactions.getState().rollback(connId);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [activeConnId]);

  // Block window close when there are open transactions.
  useEffect(() => {
    const win = getCurrentWindow();
    const unlistenPromise = win.onCloseRequested((event) => {
      const active = Object.values(useTransactions.getState().byConn).filter(
        (t) => t.active,
      );
      if (active.length === 0) return;
      event.preventDefault();
      void (async () => {
        const choice = await openConfirmModal({
          title: "Open transactions",
          body: `${active.length} transaction(s) have uncommitted changes.`,
          buttons: ["Commit all", "Rollback all", "Cancel"],
        });
        try {
          if (choice === "Commit all") {
            for (const t of active) {
              await useTransactions.getState().commit(t.connId);
            }
            await win.close();
          } else if (choice === "Rollback all") {
            for (const t of active) {
              await useTransactions.getState().rollback(t.connId);
            }
            await win.close();
          }
          // Cancel / dismiss: stay open.
        } catch (e) {
          const msg = e instanceof Error ? e.message : String(e);
          toast.error(`Failed to finalize transactions: ${msg}`);
        }
      })();
    });
    return () => {
      void unlistenPromise.then((fn) => fn());
    };
  }, []);

  return (
    <div className="bg-background text-foreground grid h-full grid-cols-[280px_1fr]">
      <ConfirmModalHost />
      <aside className="border-border flex flex-col border-r">
        <div className="flex items-center justify-between p-3">
          <h1 className="text-lg font-semibold">Tusk</h1>
          <Button variant="ghost" size="icon-sm" onClick={toggle}>
            {theme === "light" ? <Moon /> : <Sun />}
          </Button>
        </div>
        <div className="border-border flex max-h-72 flex-col gap-2 overflow-y-auto border-b p-3">
          <ConnectionForm />
          <ConnectionList />
        </div>
        <SchemaTree />
        <div className="border-border border-t p-3 text-xs">
          <label className="flex items-center justify-between gap-2">
            <span className="text-muted-foreground">Auto LIMIT</span>
            <input
              type="number"
              min={0}
              step={100}
              className="border-input w-24 rounded border px-2 py-1"
              value={autoLimit}
              onChange={(e) =>
                setAutoLimit(Math.max(0, Number(e.target.value) || 0))
              }
            />
          </label>
          <p className="text-muted-foreground mt-1">
            0 = off. Skipped if SQL has its own LIMIT.
          </p>
        </div>
      </aside>

      <main className="flex min-h-0 flex-col">
        {activeConnId && (
          <div className="border-border flex items-center gap-3 border-b px-3 py-1.5">
            <AutoCommitToggle connId={activeConnId} />
            <TxIndicator connId={activeConnId} />
          </div>
        )}
        <div className="flex min-h-0 flex-1">
          <div className="flex min-h-0 flex-1 flex-col">
            <EditorPane />
          </div>
          {activeConnId && <TxSidePanel connId={activeConnId} />}
        </div>
      </main>
    </div>
  );
}

export default App;
