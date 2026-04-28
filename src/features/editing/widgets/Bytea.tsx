import { useState } from "react";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

function b64ToHex(b64: string): string {
  try {
    const bin = atob(b64);
    let out = "";
    for (let i = 0; i < bin.length; i++) {
      out += bin.charCodeAt(i).toString(16).padStart(2, "0");
    }
    return out;
  } catch {
    return "";
  }
}

function hexToB64(hex: string): string {
  const clean = hex.replace(/\s+/g, "").replace(/^\\?x/, "");
  if (!/^[0-9a-fA-F]*$/.test(clean) || clean.length % 2 !== 0) {
    throw new Error("invalid hex");
  }
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  let bin = "";
  bytes.forEach((b) => {
    bin += String.fromCharCode(b);
  });
  return btoa(bin);
}

export function ByteaWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const isNull = initial.kind === "Null";
  const initB64 = initial.kind === "Bytea" ? initial.value.b64 : "";
  const [mode, setMode] = useState<"hex" | "b64">("hex");
  const [val, setVal] = useState(() =>
    mode === "hex" ? b64ToHex(initB64) : initB64,
  );
  const [err, setErr] = useState<string | null>(null);
  const submit = () => {
    try {
      const b64 = mode === "hex" ? hexToB64(val) : val;
      onCommit({ kind: "Bytea", value: { b64 } });
    } catch (e) {
      setErr(String(e));
    }
  };
  const onModeChange = (next: "hex" | "b64") => {
    // Convert current value to the new representation if possible.
    try {
      if (mode === "hex" && next === "b64") {
        setVal(hexToB64(val));
      } else if (mode === "b64" && next === "hex") {
        setVal(b64ToHex(val));
      }
      setErr(null);
    } catch {
      // Leave val as-is; user can fix manually.
    }
    setMode(next);
  };
  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1">
        <select
          value={mode}
          onChange={(e) => onModeChange(e.target.value as "hex" | "b64")}
          className="bg-background border-input rounded-sm border px-2 py-1 text-xs"
        >
          <option value="hex">hex</option>
          <option value="b64">base64</option>
        </select>
        <input
          autoFocus
          value={val}
          placeholder={
            isNull ? "(was NULL — type bytes to overwrite)" : undefined
          }
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
          className="bg-background border-input w-[280px] rounded-sm border px-2 py-1 font-mono text-xs"
        />
        <button
          type="button"
          onClick={submit}
          className="rounded-sm border px-2 py-1 text-xs"
        >
          OK
        </button>
        {nullable && <SetNullButton onCommit={onCommit} />}
      </div>
      {err && <span className="text-xs text-red-500">{err}</span>}
    </div>
  );
}
