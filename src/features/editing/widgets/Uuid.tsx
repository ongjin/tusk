import { useState } from "react";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

const UUID_RE =
  /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/;

export function UuidWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const isNull = initial.kind === "Null";
  const [val, setVal] = useState(initial.kind === "Uuid" ? initial.value : "");
  const [err, setErr] = useState<string | null>(null);
  const submit = () => {
    if (!UUID_RE.test(val)) {
      setErr("invalid uuid");
      return;
    }
    onCommit({ kind: "Uuid", value: val });
  };
  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1">
        <input
          autoFocus
          value={val}
          placeholder={isNull ? "(was NULL — type or Generate)" : undefined}
          onChange={(e) => {
            setVal(e.target.value);
            setErr(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              onCancel();
            }
            if (e.key === "Enter") {
              e.preventDefault();
              submit();
            }
          }}
          className="bg-background border-input rounded-sm border px-2 py-1 font-mono text-xs"
        />
        <button
          type="button"
          onClick={() => {
            setVal(crypto.randomUUID());
            setErr(null);
          }}
          className="rounded-sm border px-2 py-1 text-xs"
        >
          Generate
        </button>
        {nullable && <SetNullButton onCommit={onCommit} />}
      </div>
      {err && <span className="text-xs text-red-500">{err}</span>}
    </div>
  );
}
