import { useState } from "react";

import { renderCell } from "@/features/results/cells";
import type { Cell, ResultMeta } from "@/lib/types";
import { cn } from "@/lib/utils";
import { pkValuesOf, usePendingChanges } from "@/store/pendingChanges";

import { cellAsString } from "./cellSerde";
import { BigintWidget } from "./widgets/Bigint";
import { BoolWidget } from "./widgets/Bool";
import { ByteaWidget } from "./widgets/Bytea";
import { DateWidget } from "./widgets/Date";
import { IntWidget } from "./widgets/Int";
import { JsonWidget } from "./widgets/Json";
import { NumericWidget } from "./widgets/Numeric";
import { TextWidget } from "./widgets/Text";
import { TimeWidget } from "./widgets/Time";
import { TimestampWidget } from "./widgets/Timestamp";
import type { WidgetProps } from "./widgets/types";
import { UuidWidget } from "./widgets/Uuid";
import { VectorWidget } from "./widgets/Vector";

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
    case "date":
      return <DateWidget {...props} />;
    case "time":
      return <TimeWidget {...props} kind="Time" />;
    case "timetz":
      return <TimeWidget {...props} kind="Timetz" />;
    case "timestamp":
      return <TimestampWidget {...props} kind="Timestamp" />;
    case "timestamptz":
      return <TimestampWidget {...props} kind="Timestamptz" />;
    case "uuid":
      return <UuidWidget {...props} />;
    case "json":
    case "jsonb":
      return <JsonWidget {...props} />;
    case "bytea":
      return <ByteaWidget {...props} />;
    case "vector":
      return <VectorWidget {...props} />;
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

  const colTypeName = meta.columnTypes[columnIndex]?.typeName;
  const isReadonlyType = colTypeName === "vector" || colTypeName === "unknown";
  if (!meta.editable || isReadonlyType) {
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
