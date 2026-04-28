import { useState } from "react";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

export function BigintWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const [val, setVal] = useState(
    initial.kind === "Bigint" ? initial.value : "",
  );
  const [err, setErr] = useState<string | null>(null);

  const submit = () => {
    if (!/^-?\d+$/.test(val)) {
      setErr("integer required");
      return;
    }
    try {
      const big = BigInt(val);
      const min = BigInt("-9223372036854775808");
      const max = BigInt("9223372036854775807");
      if (big < min || big > max) {
        setErr("out of range for int8");
        return;
      }
      onCommit({ kind: "Bigint", value: val });
    } catch {
      setErr("invalid");
    }
  };

  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1">
        <input
          autoFocus
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
