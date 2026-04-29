import { useState } from "react";

import { renderCell } from "@/features/results/cells";
import type { Cell, ResultMeta } from "@/lib/types";
import { cn } from "@/lib/utils";
import { formatVectorSummary } from "@/lib/vector/cellRender";
import { pkValuesOf, usePendingChanges } from "@/store/pendingChanges";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

import { cellAsString } from "./cellSerde";
import { BigintWidget } from "./widgets/Bigint";
import { BoolWidget } from "./widgets/Bool";
import { ByteaWidget } from "./widgets/Bytea";
import { DateWidget } from "./widgets/Date";
import { EnumWidget } from "./widgets/Enum";
import { FkWidget } from "./widgets/Fk";
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
  connId: string;
  /**
   * When rendering a pending-insert ("ghost") row whose PK values aren't known
   * yet, the caller supplies the synthetic rowKey from the pending change so
   * commits update that entry instead of colliding on `JSON.stringify([])`.
   */
  rowKeyOverride?: string;
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

export function EditableCell({
  value,
  columnIndex,
  row,
  meta,
  connId,
  rowKeyOverride,
}: Props) {
  const upsertEdit = usePendingChanges((s) => s.upsertEdit);

  const columnMeta = meta.columnTypes[columnIndex];
  const columnName = columnMeta?.name ?? "";
  const pkValues = meta.editable ? pkValuesOf(meta, row) : [];
  const rowKey = rowKeyOverride
    ? rowKeyOverride
    : meta.editable
      ? JSON.stringify(pkValues)
      : "";

  const pendingEdit = usePendingChanges((s) => {
    if (!meta.editable && !rowKeyOverride) return undefined;
    const change = s.byRow.get(rowKey);
    return change?.edits.find((e) => e.column === columnName);
  });

  const display = pendingEdit?.next ?? value;
  const dirty = !!pendingEdit;

  const [editing, setEditing] = useState(false);

  const colTypeName = meta.columnTypes[columnIndex]?.typeName;
  const isReadonlyType = colTypeName === "vector" || colTypeName === "unknown";
  if (!meta.editable || isReadonlyType) {
    if (colTypeName === "vector" && value.kind === "Vector") {
      return <VectorRawCell value={value.value.values} dim={value.value.dim} />;
    }
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
          rowKey: rowKeyOverride,
        });
      }
      setEditing(false);
    };
    const onCancel = () => setEditing(false);
    const widgetProps: WidgetProps = {
      initial: display,
      nullable: columnMeta.nullable,
      onCommit,
      onCancel,
    };
    if (columnMeta.enumValues && columnMeta.enumValues.length > 0) {
      return (
        <EnumWidget
          {...widgetProps}
          enumValues={columnMeta.enumValues}
          typeName={columnMeta.typeName}
        />
      );
    }
    if (columnMeta.fk) {
      const originalKind = ((): "Int" | "Bigint" | "Text" | "Uuid" => {
        switch (columnMeta.typeName) {
          case "int2":
          case "int4":
            return "Int";
          case "int8":
            return "Bigint";
          case "uuid":
            return "Uuid";
          default:
            return "Text";
        }
      })();
      return (
        <FkWidget
          {...widgetProps}
          connId={connId}
          fk={columnMeta.fk}
          originalKind={originalKind}
        />
      );
    }
    return renderWidget(columnMeta.typeName, widgetProps);
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

function VectorRawCell({ value, dim }: { value: number[]; dim: number }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <span
        onDoubleClick={() => setOpen(true)}
        className="block w-full cursor-text"
      >
        {renderCell({ kind: "Vector", value: { dim, values: value } })}
      </span>
      {open && (
        <Dialog open onOpenChange={(o) => !o && setOpen(false)}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Vector value</DialogTitle>
            </DialogHeader>
            <div className="text-muted-foreground text-xs">
              {formatVectorSummary(value)}
            </div>
            <pre className="bg-muted max-h-[60vh] overflow-auto rounded p-2 text-[10px]">
              [{value.join(", ")}]
            </pre>
            <DialogFooter>
              <Button
                onClick={() =>
                  navigator.clipboard.writeText(`[${value.join(",")}]`)
                }
              >
                Copy
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </>
  );
}
