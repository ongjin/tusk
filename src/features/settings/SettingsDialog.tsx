import { useState } from "react";
import { Dialog as DialogPrimitive } from "radix-ui";

import { ProviderSection } from "./ProviderSection";
import { SchemaIndexPanel } from "./SchemaIndexPanel";

type Tab = "general" | "providers" | "schema-index" | "advanced";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialTab?: Tab;
}

export function SettingsDialog({ open, onOpenChange, initialTab }: Props) {
  const [tab, setTab] = useState<Tab>(initialTab ?? "providers");
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <DialogPrimitive.Content className="bg-card fixed top-1/2 left-1/2 z-50 flex h-[80vh] w-[720px] -translate-x-1/2 -translate-y-1/2 flex-col rounded border shadow">
          <DialogPrimitive.Title className="border-border border-b px-4 py-3 text-sm font-medium">
            Settings
          </DialogPrimitive.Title>
          <div className="flex min-h-0 flex-1">
            <nav className="border-border w-44 border-r p-2 text-xs">
              {(
                [
                  ["general", "General"],
                  ["providers", "Providers"],
                  ["schema-index", "Schema Index"],
                  ["advanced", "Advanced"],
                ] as const
              ).map(([k, label]) => (
                <button
                  key={k}
                  type="button"
                  onClick={() => setTab(k)}
                  className={`w-full rounded px-2 py-1 text-left ${
                    tab === k ? "bg-accent" : "hover:bg-accent/50"
                  }`}
                >
                  {label}
                </button>
              ))}
            </nav>
            <div className="flex-1 overflow-y-auto p-4">
              {tab === "providers" && <ProviderSection />}
              {tab === "general" && (
                <p className="text-muted-foreground text-xs">
                  General settings — coming later.
                </p>
              )}
              {tab === "schema-index" && <SchemaIndexPanel />}
              {tab === "advanced" && (
                <p className="text-muted-foreground text-xs">Reserved.</p>
              )}
            </div>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
