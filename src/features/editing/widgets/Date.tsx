import { useState } from "react";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

export function DateWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const isNull = initial.kind === "Null";
  const [val, setVal] = useState(initial.kind === "Date" ? initial.value : "");
  return (
    <div className="flex items-center gap-1">
      <input
        type="date"
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
            onCommit({ kind: "Date", value: val });
          }
        }}
        className="bg-background border-input rounded-sm border px-2 py-1 text-xs"
      />
      <button
        type="button"
        onClick={() => onCommit({ kind: "Date", value: val })}
        className="rounded-sm border px-2 py-1 text-xs"
      >
        OK
      </button>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
