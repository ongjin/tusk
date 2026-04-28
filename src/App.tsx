import { useState } from "react";
import { Moon, Play, Sun } from "lucide-react";
import { toast } from "sonner";

import { ConnectionForm } from "@/features/connections/ConnectionForm";
import { ConnectionList } from "@/features/connections/ConnectionList";
import { Button } from "@/components/ui/button";
import { useTheme } from "@/hooks/use-theme";
import { useConnections } from "@/store/connections";
import { executeQuery } from "@/lib/tauri";
import type { QueryResult } from "@/lib/types";

function App() {
  const { theme, toggle } = useTheme();
  const activeId = useConnections((s) => s.activeId);
  const [sql, setSql] = useState("SELECT 1");
  const [result, setResult] = useState<QueryResult | null>(null);
  const [busy, setBusy] = useState(false);

  async function run() {
    if (!activeId) {
      toast.error("Select a connected database first");
      return;
    }
    setBusy(true);
    try {
      const r = await executeQuery(activeId, sql);
      setResult(r);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Query failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="bg-background text-foreground grid h-full grid-cols-[280px_1fr]">
      <aside className="border-border flex flex-col gap-3 border-r p-3">
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-semibold">Tusk</h1>
          <Button variant="ghost" size="icon-sm" onClick={toggle}>
            {theme === "light" ? <Moon /> : <Sun />}
          </Button>
        </div>
        <ConnectionForm />
        <ConnectionList />
      </aside>

      <main className="flex flex-col gap-3 p-4">
        <div className="flex items-center gap-2">
          <textarea
            className="border-input bg-background min-h-[120px] flex-1 rounded-md border px-3 py-2 font-mono text-sm"
            value={sql}
            onChange={(e) => setSql(e.target.value)}
            spellCheck={false}
          />
          <Button onClick={run} disabled={busy}>
            <Play />
            Run
          </Button>
        </div>
        {result && (
          <pre className="bg-muted/40 max-h-[60vh] overflow-auto rounded-md p-3 text-xs">
            {JSON.stringify(result, null, 2)}
          </pre>
        )}
      </main>
    </div>
  );
}

export default App;
