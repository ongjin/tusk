import { Plus, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useTabs } from "@/store/tabs";
import { cn } from "@/lib/utils";

export function EditorTabs() {
  const tabs = useTabs((s) => s.tabs);
  const activeId = useTabs((s) => s.activeId);
  const setActive = useTabs((s) => s.setActive);
  const newTab = useTabs((s) => s.newTab);
  const closeTab = useTabs((s) => s.closeTab);

  return (
    <div className="border-border bg-muted/30 flex items-center gap-1 border-b px-2 py-1">
      {tabs.map((t) => (
        <button
          key={t.id}
          onClick={() => setActive(t.id)}
          className={cn(
            "group flex items-center gap-1 rounded px-2 py-1 text-xs",
            t.id === activeId ? "bg-background border" : "hover:bg-accent",
          )}
        >
          <span>
            {t.title}
            {t.dirty && "•"}
          </span>
          <span
            role="button"
            className="rounded p-0.5 opacity-50 hover:opacity-100"
            onClick={(e) => {
              e.stopPropagation();
              closeTab(t.id);
            }}
          >
            <X className="size-3" />
          </span>
        </button>
      ))}
      <Button size="icon-xs" variant="ghost" onClick={() => newTab(null)}>
        <Plus />
      </Button>
    </div>
  );
}
