import { useState } from "react";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

export function NumericWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const isNull = initial.kind === "Null";
  const [val, setVal] = useState(
    initial.kind === "Numeric" ? initial.value : "",
  );
  const [err, setErr] = useState<string | null>(null);

  const submit = () => {
    if (!/^-?\d+(\.\d+)?$/.test(val)) {
      setErr("numeric required");
      return;
    }
    onCommit({ kind: "Numeric", value: val });
  };

  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1">
        <input
          autoFocus
          placeholder={isNull ? "(was NULL — type to overwrite)" : undefined}
          value={val}
          onChange={(e) => {
            setVal(e.target.value);
            setErr(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              onCancel();
            } else if (e.key === "Enter") {
              e.preventDefault();
              submit();
            }
          }}
          className="bg-background border-input flex-1 rounded-sm border px-1 py-0 font-mono text-xs outline-none focus:ring-1 focus:ring-amber-500"
        />
        {nullable && <SetNullButton onCommit={onCommit} />}
      </div>
      {err && <span className="text-xs text-red-500">{err}</span>}
    </div>
  );
}
