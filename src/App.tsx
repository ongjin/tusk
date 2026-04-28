import { Moon, Sun } from "lucide-react";

import { ConnectionForm } from "@/features/connections/ConnectionForm";
import { ConnectionList } from "@/features/connections/ConnectionList";
import { SchemaTree } from "@/features/schema/SchemaTree";
import { EditorPane } from "@/features/editor/EditorPane";
import { Button } from "@/components/ui/button";
import { useTheme } from "@/hooks/use-theme";

function App() {
  const { theme, toggle } = useTheme();

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
      </aside>

      <main className="flex min-h-0 flex-col">
        <EditorPane />
      </main>
    </div>
  );
}

export default App;
