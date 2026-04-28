import { useState } from "react";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

export function IntWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const isNull = initial.kind === "Null";
  const [val, setVal] = useState(
    initial.kind === "Int" ? String(initial.value) : "",
  );
  const [err, setErr] = useState<string | null>(null);

  const submit = () => {
    if (!/^-?\d+$/.test(val)) {
      setErr("integer required");
      return;
    }
    const n = Number(val);
    if (n < -2147483648 || n > 2147483647) {
      setErr("out of range for int4");
      return;
    }
    onCommit({ kind: "Int", value: n });
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
