import Editor from "@monaco-editor/react";
import { useState } from "react";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

export function JsonWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const [val, setVal] = useState(() => {
    if (initial.kind !== "Json") return "{}";
    return JSON.stringify(initial.value, null, 2);
  });
  const [err, setErr] = useState<string | null>(null);
  const submit = () => {
    try {
      onCommit({ kind: "Json", value: JSON.parse(val) });
    } catch (e) {
      setErr(String(e));
    }
  };
  return (
    <div className="flex h-[180px] w-[360px] flex-col">
      <Editor
        height="140px"
        language="json"
        value={val}
        onChange={(v) => {
          setVal(v ?? "");
          setErr(null);
        }}
        options={{ minimap: { enabled: false }, fontSize: 12 }}
      />
      <div className="mt-1 flex items-center gap-1">
        <button
          type="button"
          onClick={submit}
          className="rounded-sm border px-2 py-1 text-xs"
        >
          OK
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-sm border px-2 py-1 text-xs"
        >
          Cancel
        </button>
        {nullable && <SetNullButton onCommit={onCommit} />}
        {err && <span className="ml-2 text-xs text-red-500">{err}</span>}
      </div>
    </div>
  );
}
