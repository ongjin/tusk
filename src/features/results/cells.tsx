import { useEffect, useRef } from "react";
import type { ReactNode } from "react";

import type { Cell } from "@/lib/types";
import {
  formatVectorSummary,
  l2Norm,
  renderSparkline,
} from "@/lib/vector/cellRender";

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
      return <VectorCell vec={cell.value.values} />;
    case "Unknown":
      return (
        <span className="text-muted-foreground italic">
          {cell.value.text || `<oid ${cell.value.oid}>`}
        </span>
      );
  }
}

function VectorCell({ vec }: { vec: number[] }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (ref.current) renderSparkline(ref.current, vec);
  }, [vec]);
  return (
    <span
      className="inline-flex items-center gap-1 font-mono text-xs"
      title={`dim=${vec.length}, ‖v‖=${l2Norm(vec).toFixed(4)}`}
    >
      <canvas ref={ref} width={48} height={12} className="text-blue-500" />
      <span className="text-muted-foreground">{formatVectorSummary(vec)}</span>
    </span>
  );
}
