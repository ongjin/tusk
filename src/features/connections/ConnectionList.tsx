// src/features/connections/ConnectionList.tsx
import { useEffect } from "react";
import { Plug, PlugZap, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { useConnections } from "@/store/connections";
import { cn } from "@/lib/utils";

export function ConnectionList() {
  const items = useConnections((s) => s.items);
  const activeId = useConnections((s) => s.activeId);
  const refresh = useConnections((s) => s.refresh);
  const connect = useConnections((s) => s.connect);
  const disconnect = useConnections((s) => s.disconnect);
  const remove = useConnections((s) => s.remove);
  const setActive = useConnections((s) => s.setActive);

  useEffect(() => {
    refresh().catch((e) =>
      toast.error(`Failed to load connections: ${e.message ?? e}`),
    );
  }, [refresh]);

  if (items.length === 0) {
    return (
      <p className="text-muted-foreground px-3 py-2 text-sm">
        No connections yet — click <kbd>+ New connection</kbd> to add one.
      </p>
    );
  }

  return (
    <ul className="flex flex-col gap-1">
      {items.map((c) => {
        const isActive = activeId === c.id;
        return (
          <li
            key={c.id}
            className={cn(
              "group flex items-center justify-between rounded-md border px-3 py-2",
              isActive && "border-primary",
            )}
            onClick={() => setActive(c.id)}
          >
            <div>
              <div className="flex items-center gap-2 text-sm font-medium">
                <span
                  className={cn(
                    "size-2 rounded-full",
                    c.connected ? "bg-emerald-500" : "bg-muted-foreground/40",
                  )}
                />
                {c.name}
              </div>
              <div className="text-muted-foreground text-xs">
                {c.dbUser}@{c.host}:{c.port}/{c.database}
              </div>
            </div>
            <div className="flex gap-1 opacity-0 group-hover:opacity-100">
              {c.connected ? (
                <Button
                  size="icon-sm"
                  variant="ghost"
                  onClick={(e) => {
                    e.stopPropagation();
                    disconnect(c.id).catch((err) => toast.error(err.message));
                  }}
                >
                  <PlugZap />
                </Button>
              ) : (
                <Button
                  size="icon-sm"
                  variant="ghost"
                  onClick={(e) => {
                    e.stopPropagation();
                    connect(c.id)
                      .then(() => toast.success(`Connected to ${c.name}`))
                      .catch((err) => toast.error(err.message));
                  }}
                >
                  <Plug />
                </Button>
              )}
              <Button
                size="icon-sm"
                variant="ghost"
                onClick={(e) => {
                  e.stopPropagation();
                  remove(c.id).catch((err) => toast.error(err.message));
                }}
              >
                <Trash2 />
              </Button>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
