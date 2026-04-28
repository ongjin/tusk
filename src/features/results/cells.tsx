import type { ReactNode } from "react";

import type { Cell } from "@/lib/types";

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
    case "Bytea":
      return (
        <span className="font-mono text-xs">
          {`\\x${cell.value.b64.slice(0, 24)}…`}
        </span>
      );
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
