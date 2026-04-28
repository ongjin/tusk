import { Moon, Sun } from "lucide-react";

import { ConnectionForm } from "@/features/connections/ConnectionForm";
import { ConnectionList } from "@/features/connections/ConnectionList";
import { SchemaTree } from "@/features/schema/SchemaTree";
import { EditorPane } from "@/features/editor/EditorPane";
import { Button } from "@/components/ui/button";
import { useTheme } from "@/hooks/use-theme";
import { useSettings } from "@/store/settings";

function App() {
  const { theme, toggle } = useTheme();
  const autoLimit = useSettings((s) => s.autoLimit);
  const setAutoLimit = useSettings((s) => s.setAutoLimit);

  return (
    <div className="bg-background text-foreground grid h-full grid-cols-[280px_1fr]">
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
        <EditorPane />
      </main>
    </div>
  );
}

export default App;
