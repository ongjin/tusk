import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { Cell } from "@/lib/types";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

interface FkOption {
  pkValue: string;
  display: string;
}

export function FkWidget({
  nullable,
  onCommit,
  onCancel,
  connId,
  fk,
  originalKind,
}: WidgetProps & {
  connId: string;
  fk: { schema: string; table: string; column: string };
  originalKind: "Int" | "Bigint" | "Text" | "Uuid";
}) {
  const [q, setQ] = useState("");
  const [opts, setOpts] = useState<FkOption[]>([]);

  useEffect(() => {
    const t = setTimeout(() => {
      invoke<FkOption[]>("fk_lookup", {
        connectionId: connId,
        schema: fk.schema,
        table: fk.table,
        pkColumn: fk.column,
        query: q,
      })
        .then(setOpts)
        .catch(() => setOpts([]));
    }, 150);
    return () => clearTimeout(t);
  }, [q, connId, fk.schema, fk.table, fk.column]);

  const commit = (raw: string) => {
    const c: Cell = (() => {
      switch (originalKind) {
        case "Int":
          return { kind: "Int", value: Number(raw) };
        case "Bigint":
          return { kind: "Bigint", value: raw };
        case "Uuid":
          return { kind: "Uuid", value: raw };
        default:
          return { kind: "Text", value: raw };
      }
    })();
    onCommit(c);
  };

  return (
    <div className="flex w-[320px] flex-col gap-1">
      <input
        autoFocus
        placeholder={`Search ${fk.table}.${fk.column}`}
        value={q}
        onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onCancel();
          }
        }}
        className="bg-background border-input rounded-sm border px-2 py-1 text-xs"
      />
      <div className="max-h-40 overflow-auto rounded-sm border">
        {opts.length === 0 && (
          <div className="text-muted-foreground px-2 py-1 text-xs italic">
            (no matches)
          </div>
        )}
        {opts.map((o) => (
          <button
            key={o.pkValue}
            type="button"
            className="hover:bg-muted block w-full px-2 py-0.5 text-left text-xs"
            onClick={() => commit(o.pkValue)}
          >
            <span className="font-mono">{o.pkValue}</span>
            <span className="text-muted-foreground ml-2">{o.display}</span>
          </button>
        ))}
      </div>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
