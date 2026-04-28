// src/features/connections/SshHostPicker.tsx
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { listKnownSshHosts } from "@/lib/tauri";
import type { SshHost } from "@/lib/types";
import { cn } from "@/lib/utils";

interface Props {
  selectedAlias: string | null;
  onSelect: (host: SshHost) => void;
}

export function SshHostPicker({ selectedAlias, onSelect }: Props) {
  const [hosts, setHosts] = useState<SshHost[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    listKnownSshHosts()
      .then(setHosts)
      .catch((e) => toast.error(`SSH config: ${e.message ?? e}`))
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <p className="text-muted-foreground text-xs">Loading…</p>;

  if (hosts.length === 0) {
    return (
      <p className="text-muted-foreground text-xs">
        No usable hosts in <code>~/.ssh/config</code>.
      </p>
    );
  }

  return (
    <ul className="border-input max-h-48 overflow-auto rounded-md border">
      {hosts.map((h) => {
        const isActive = selectedAlias === h.alias;
        return (
          <li
            key={h.alias}
            className={cn(
              "hover:bg-accent cursor-pointer border-b px-3 py-2 text-sm last:border-b-0",
              isActive && "bg-accent",
            )}
            onClick={() => onSelect(h)}
          >
            <div className="font-medium">{h.alias}</div>
            <div className="text-muted-foreground text-xs">
              {h.user ?? "?"}@{h.hostname ?? "?"}
              {h.proxyJump ? ` · via ${h.proxyJump}` : ""}
            </div>
          </li>
        );
      })}
    </ul>
  );
}
