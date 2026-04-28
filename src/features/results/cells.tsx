import type { ReactNode } from "react";

import type { Cell } from "@/lib/types";

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

export function renderCell(cell: Cell): ReactNode {
  switch (cell.kind) {
    case "Null":
      return <span className="text-muted-foreground italic">NULL</span>;
    case "Bool":
      return cell.value ? "true" : "false";
    case "Int":
    case "Float":
      return String(cell.value);
    case "Bigint":
    case "Numeric":
    case "Text":
    case "Uuid":
    case "Inet":
    case "Date":
    case "Time":
    case "Timetz":
    case "Timestamp":
    case "Timestamptz":
      return cell.value;
    case "Interval":
      return cell.value.iso;
    case "Bytea": {
      const hex = b64ToHex(cell.value.b64);
      const truncated = hex.length > 24;
      return (
        <span className="font-mono text-xs">
          \x{truncated ? hex.slice(0, 24) : hex}
          {truncated && "…"}
        </span>
      );
    }
    case "Json":
      return (
        <code className="text-xs">
          {JSON.stringify(cell.value).slice(0, 80)}
        </code>
      );
    case "Array":
      return `{${cell.value.values.length} items}`;
    case "Enum":
      return cell.value.value;
    case "Vector":
      return `vector(${cell.value.dim})`;
    case "Unknown":
      return (
        <span className="text-muted-foreground italic">
          {cell.value.text || `<oid ${cell.value.oid}>`}
        </span>
      );
  }
}
