import { useState } from "react";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

export function TextWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const isNull = initial.kind === "Null";
  const [val, setVal] = useState(
    initial.kind === "Null" ? "" : initial.kind === "Text" ? initial.value : "",
  );
  const [multiline, setMultiline] = useState(false);

  const inputClass =
    "bg-background border-input flex-1 rounded-sm border px-1 py-0 font-mono text-xs outline-none focus:ring-1 focus:ring-amber-500";

  return (
    <div className="flex items-center gap-1">
      {multiline ? (
        <textarea
          autoFocus
          placeholder={isNull ? "(was NULL — type to overwrite)" : undefined}
          value={val}
          onChange={(e) => setVal(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              onCancel();
            } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              onCommit({ kind: "Text", value: val });
            }
          }}
          className={inputClass}
          rows={3}
        />
      ) : (
        <input
          autoFocus
          placeholder={isNull ? "(was NULL — type to overwrite)" : undefined}
          value={val}
          onChange={(e) => setVal(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              onCancel();
            } else if (e.key === "Enter") {
              e.preventDefault();
              onCommit({ kind: "Text", value: val });
            }
          }}
          className={inputClass}
        />
      )}
      <button
        type="button"
        onClick={() => setMultiline(!multiline)}
        className="border-input hover:bg-accent rounded border px-1 text-xs"
      >
        {multiline ? "single" : "multi"}
      </button>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
