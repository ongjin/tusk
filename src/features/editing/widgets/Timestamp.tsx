import { useState } from "react";

import type { Cell } from "@/lib/types";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

export function TimestampWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
  kind,
}: WidgetProps & { kind: "Timestamp" | "Timestamptz" }) {
  const isNull = initial.kind === "Null";
  const [val, setVal] = useState(() => {
    if (initial.kind === kind) return (initial.value as string).slice(0, 19);
    return "";
  });
  const tzOffsetMin =
    kind === "Timestamptz" ? new Date().getTimezoneOffset() : null;
  return (
    <div className="flex items-center gap-1">
      <input
        type="datetime-local"
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
      {tzOffsetMin !== null && (
        <span className="text-muted-foreground text-xs">
          UTC{tzOffsetMin <= 0 ? "+" : "-"}
          {Math.abs(tzOffsetMin / 60)}
        </span>
      )}
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
