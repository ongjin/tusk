import { Moon, Sun } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useTheme } from "@/hooks/use-theme";

function App() {
  const { theme, toggle } = useTheme();

  return (
    <div className="bg-background text-foreground min-h-full">
      <header className="border-border flex items-center justify-between border-b px-8 py-4">
        <div className="flex items-baseline gap-3">
          <h1 className="text-2xl font-semibold tracking-tight">Tusk</h1>
          <span className="text-muted-foreground text-sm">
            Postgres, with intelligence.
          </span>
        </div>
        <Button
          variant="outline"
          size="icon"
          aria-label="Toggle theme"
          onClick={toggle}
        >
          {theme === "light" ? <Moon /> : <Sun />}
        </Button>
      </header>

      <main className="space-y-8 px-8 py-10">
        <section className="space-y-3">
          <h2 className="text-muted-foreground text-xs font-medium tracking-widest uppercase">
            Brand
          </h2>
          <div className="flex flex-wrap items-center gap-3">
            <div className="bg-primary text-primary-foreground rounded-md px-4 py-2 text-sm font-medium">
              Tusk Amber
            </div>
            <div className="bg-secondary text-secondary-foreground rounded-md px-4 py-2 text-sm font-medium">
              Secondary
            </div>
            <div className="bg-accent text-accent-foreground rounded-md px-4 py-2 text-sm font-medium">
              Accent
            </div>
            <div className="bg-muted text-muted-foreground rounded-md px-4 py-2 text-sm font-medium">
              Muted
            </div>
          </div>
        </section>

        <section className="space-y-3">
          <h2 className="text-muted-foreground text-xs font-medium tracking-widest uppercase">
            Buttons
          </h2>
          <div className="flex flex-wrap gap-2">
            <Button>Default</Button>
            <Button variant="secondary">Secondary</Button>
            <Button variant="outline">Outline</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="link">Link</Button>
            <Button variant="destructive">Destructive</Button>
          </div>
        </section>

        <section className="space-y-3">
          <h2 className="text-muted-foreground text-xs font-medium tracking-widest uppercase">
            Status
          </h2>
          <p className="text-muted-foreground max-w-xl text-sm">
            Week 1 scaffold — Tauri 2 + React 19 + Tailwind v4 + shadcn/ui. Real
            Postgres connection lands in Week 2.
          </p>
        </section>
      </main>
    </div>
  );
}

export default App;
