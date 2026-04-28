import { useState, type KeyboardEvent } from "react";

import { renderCell } from "@/features/results/cells";
import type { Cell, ResultMeta } from "@/lib/types";
import { cn } from "@/lib/utils";
import { pkValuesOf, usePendingChanges } from "@/store/pendingChanges";

import { cellAsString, cellsEqual, parseCellLike } from "./cellSerde";

interface Props {
  value: Cell;
  columnIndex: number;
  row: Cell[];
  meta: ResultMeta;
}

export function EditableCell({ value, columnIndex, row, meta }: Props) {
  const upsertEdit = usePendingChanges((s) => s.upsertEdit);

  const columnName = meta.columnTypes[columnIndex]?.name ?? "";
  const pkValues = meta.editable ? pkValuesOf(meta, row) : [];
  const rowKey = meta.editable ? JSON.stringify(pkValues) : "";

  const pendingEdit = usePendingChanges((s) => {
    if (!meta.editable) return undefined;
    const change = s.byRow.get(rowKey);
    return change?.edits.find((e) => e.column === columnName);
  });

  const display = pendingEdit?.next ?? value;
  const dirty = !!pendingEdit;

  const [editing, setEditing] = useState(false);

  if (!meta.editable) {
    return <>{renderCell(value)}</>;
  }

  if (editing) {
    const initial = cellAsString(display);
    const onSave = (text: string) => {
      const next = parseCellLike(text, value);
      // Only register an edit when it actually differs from the current row value.
      if (!cellsEqual(next, value)) {
        if (meta.table) {
          upsertEdit({
            table: meta.table,
            pkColumns: meta.pkColumns,
            pkValues,
            column: columnName,
            original: value,
            next,
            capturedRow: row,
            capturedColumns: meta.columnTypes.map((c) => c.name),
          });
        }
      }
      setEditing(false);
    };
    const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Escape") {
        e.preventDefault();
        setEditing(false);
      } else if (e.key === "Enter") {
        e.preventDefault();
        (e.target as HTMLInputElement).blur();
      }
    };
    return (
      <input
        autoFocus
        defaultValue={initial}
        onBlur={(e) => onSave(e.target.value)}
        onKeyDown={onKey}
        className="bg-background border-input w-full rounded-sm border px-1 py-0 font-mono text-xs outline-none focus:ring-1 focus:ring-amber-500"
      />
    );
  }

  return (
    <span
      onDoubleClick={() => setEditing(true)}
      title={
        dirty
          ? `Original: ${cellAsString(pendingEdit!.original) || "(empty)"}`
          : undefined
      }
      className={cn(
        "block w-full cursor-text",
        dirty && "-mx-3 -my-1 bg-amber-500/20 px-3 py-1 dark:bg-amber-400/20",
      )}
    >
      {renderCell(display)}
    </span>
  );
}
