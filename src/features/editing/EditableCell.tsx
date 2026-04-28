import { useState } from "react";

import { renderCell } from "@/features/results/cells";
import type { Cell, ResultMeta } from "@/lib/types";
import { cn } from "@/lib/utils";
import { pkValuesOf, usePendingChanges } from "@/store/pendingChanges";

import { cellAsString } from "./cellSerde";
import { BigintWidget } from "./widgets/Bigint";
import { BoolWidget } from "./widgets/Bool";
import { IntWidget } from "./widgets/Int";
import { NumericWidget } from "./widgets/Numeric";
import { TextWidget } from "./widgets/Text";
import type { WidgetProps } from "./widgets/types";

interface Props {
  value: Cell;
  columnIndex: number;
  row: Cell[];
  meta: ResultMeta;
}

function renderWidget(typeName: string, props: WidgetProps) {
  switch (typeName) {
    case "int2":
    case "int4":
      return <IntWidget {...props} />;
    case "int8":
      return <BigintWidget {...props} />;
    case "numeric":
      return <NumericWidget {...props} />;
    case "bool":
      return <BoolWidget {...props} />;
    case "text":
    case "varchar":
    case "bpchar":
    default:
      return <TextWidget {...props} />;
  }
}

export function EditableCell({ value, columnIndex, row, meta }: Props) {
  const upsertEdit = usePendingChanges((s) => s.upsertEdit);

  const columnMeta = meta.columnTypes[columnIndex];
  const columnName = columnMeta?.name ?? "";
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

  if (editing && columnMeta) {
    const onCommit = (next: Cell) => {
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
      setEditing(false);
    };
    const onCancel = () => setEditing(false);
    return renderWidget(columnMeta.typeName, {
      initial: display,
      nullable: columnMeta.nullable,
      onCommit,
      onCancel,
    });
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
