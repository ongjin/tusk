import { useState } from "react";

import type { Cell } from "@/lib/types";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

export function TimeWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
  kind,
}: WidgetProps & { kind: "Time" | "Timetz" }) {
  const isNull = initial.kind === "Null";
  const [val, setVal] = useState(
    initial.kind === kind ? (initial.value as string) : "",
  );
  return (
    <div className="flex items-center gap-1">
      <input
        type="time"
        step="1"
        autoFocus
        value={val}
        placeholder={isNull ? "(was NULL)" : undefined}
        onChange={(e) => setVal(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onCancel();
          }
          if (e.key === "Enter") {
            e.preventDefault();
            onCommit({ kind, value: val } as Cell);
          }
        }}
        className="bg-background border-input rounded-sm border px-2 py-1 text-xs"
      />
      <button
        type="button"
        onClick={() => onCommit({ kind, value: val } as Cell)}
        className="rounded-sm border px-2 py-1 text-xs"
      >
        OK
      </button>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
