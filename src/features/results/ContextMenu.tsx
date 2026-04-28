import { useEffect, useRef } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "sonner";

import type { Cell, ResultMeta } from "@/lib/types";
import { toLiteral } from "@/lib/pgLiterals";
import { pkValuesOf, usePendingChanges } from "@/store/pendingChanges";

interface Props {
  cell: Cell;
  columnIndex: number;
  row: Cell[];
  meta: ResultMeta;
  x: number;
  y: number;
  onClose: () => void;
  onFilter: (col: string, value: Cell) => void;
}

export function CellContextMenu({
  cell,
  columnIndex,
  row,
  meta,
  x,
  y,
  onClose,
  onFilter,
}: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const colMeta = meta.columnTypes[columnIndex];
  const colName = colMeta?.name ?? "";
  const nullable = colMeta?.nullable ?? false;

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  const copyText = async () => {
    await writeText(cellToText(cell));
    onClose();
  };

  const copyAsInsert = async () => {
    if (!meta.table) {
      onClose();
      return;
    }
    const cols = meta.columnTypes.map((c) => `"${c.name}"`).join(", ");
    const literals = row.map(toLiteral).join(", ");
    const text = `INSERT INTO "${meta.table.schema}"."${meta.table.name}" (${cols}) VALUES (${literals});`;
    await writeText(text);
    toast.success("Copied INSERT to clipboard");
    onClose();
  };

  const setNull = () => {
    if (!nullable || !meta.table) {
      onClose();
      return;
    }
    usePendingChanges.getState().upsertEdit({
      table: meta.table,
      pkColumns: meta.pkColumns,
      pkValues: pkValuesOf(meta, row),
      column: colName,
      original: cell,
      next: { kind: "Null" },
      capturedRow: row,
      capturedColumns: meta.columnTypes.map((c) => c.name),
    });
    onClose();
  };

  const editable = !!meta.table && meta.editable;

  return (
    <div
      ref={ref}
      style={{ left: x, top: y }}
      className="bg-card fixed z-50 min-w-[10rem] rounded-sm border text-xs shadow-md"
    >
      <button
        type="button"
        onClick={copyText}
        className="hover:bg-muted block w-full px-3 py-1 text-left"
      >
        Copy
      </button>
      <button
        type="button"
        onClick={copyAsInsert}
        disabled={!meta.table}
        className="hover:bg-muted block w-full px-3 py-1 text-left disabled:opacity-50"
      >
        Copy as INSERT
      </button>
      {nullable && editable && (
        <button
          type="button"
          onClick={setNull}
          className="hover:bg-muted block w-full px-3 py-1 text-left"
        >
          Set NULL
        </button>
      )}
      <button
        type="button"
        onClick={() => {
          onFilter(colName, cell);
          onClose();
        }}
        className="hover:bg-muted block w-full px-3 py-1 text-left"
      >
        Filter by this value
      </button>
    </div>
  );
}

function cellToText(c: Cell): string {
  switch (c.kind) {
    case "Null":
      return "NULL";
    case "Bool":
      return c.value ? "true" : "false";
    case "Int":
    case "Float":
      return String(c.value);
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
      return c.value;
    case "Interval":
      return c.value.iso;
    case "Bytea": {
      // Match export.rs / pgLiterals.ts hex form so users can paste back into
      // a bytea column. Falls back to the raw b64 if decoding throws.
      try {
        const bin = atob(c.value.b64);
        let hex = "";
        for (let i = 0; i < bin.length; i++) {
          hex += bin.charCodeAt(i).toString(16).padStart(2, "0");
        }
        return `\\x${hex}`;
      } catch {
        return `\\x${c.value.b64}`;
      }
    }
    case "Json":
      return JSON.stringify(c.value);
    case "Array":
      return JSON.stringify(c.value.values);
    case "Enum":
      return c.value.value;
    case "Vector":
      return JSON.stringify(c.value.values);
    case "Unknown":
      return c.value.text;
  }
}
