import { useMemo, useRef, useState } from "react";
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";

import { EditableCell } from "@/features/editing/EditableCell";
import type { Cell, QueryResult } from "@/lib/types";
import { toLiteral } from "@/lib/pgLiterals";
import { usePendingChanges } from "@/store/pendingChanges";
import { useTabs } from "@/store/tabs";

import { CellContextMenu } from "./ContextMenu";

type Row = Record<string, Cell>;

export function ResultsGrid({
  result,
  connId,
}: {
  result: QueryResult;
  connId: string;
}) {
  const data = useMemo<Row[]>(
    () =>
      result.rows.map((row) => {
        const obj: Row = {};
        result.columns.forEach((c, i) => (obj[c.name] = row[i]));
        return obj;
      }),
    [result],
  );

  const columns = useMemo<ColumnDef<Row>[]>(
    () =>
      result.columns.map((c, columnIndex) => ({
        accessorKey: c.name,
        header: () => (
          <div className="flex flex-col leading-tight">
            <span className="text-foreground text-xs font-medium">
              {c.name}
            </span>
            <span className="text-muted-foreground text-[10px]">
              {c.typeName}
            </span>
          </div>
        ),
        cell: (info) => (
          <EditableCell
            value={info.getValue() as Cell}
            columnIndex={columnIndex}
            row={result.rows[info.row.index]}
            meta={result.meta}
            connId={connId}
          />
        ),
      })),
    [result.columns, result.rows, result.meta, connId],
  );

  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const parentRef = useRef<HTMLDivElement>(null);
  const rowVirtualizer = useVirtualizer({
    count: table.getRowModel().rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 28,
    overscan: 12,
  });

  const pendingInserts = usePendingChanges((s) =>
    s
      .list()
      .filter(
        (p) =>
          p.op === "insert" &&
          result.meta.table &&
          p.table.schema === result.meta.table.schema &&
          p.table.name === result.meta.table.name,
      ),
  );

  const [menu, setMenu] = useState<{
    cell: Cell;
    columnIndex: number;
    row: Cell[];
    x: number;
    y: number;
  } | null>(null);

  const onFilter = (col: string, value: Cell) => {
    const tabs = useTabs.getState();
    const activeId = tabs.activeId;
    const tab = tabs.tabs.find((t) => t.id === activeId);
    if (!tab) return;
    const literal = toLiteral(value);
    const clause = `WHERE "${col}" = ${literal}`;
    const trimmed = tab.sql.trimEnd();
    const sep = trimmed.length === 0 ? "" : "\n";
    tabs.updateSql(activeId, `${trimmed}${sep}${clause}`);
  };

  return (
    <div ref={parentRef} className="flex-1 overflow-auto font-mono text-xs">
      {pendingInserts.length > 0 && (
        <div className="border-b-2 border-amber-500/40">
          {pendingInserts.map((p) => (
            <div
              key={p.rowKey}
              className="flex items-center gap-2 bg-amber-500/10 px-2 py-1"
            >
              <span className="shrink-0 text-xs text-amber-600 dark:text-amber-400">
                + row
              </span>
              <div className="flex flex-1 flex-wrap items-center gap-2">
                {result.meta.columnTypes.map((col, idx) => {
                  const cellValue =
                    p.edits.find((e) => e.column === col.name)?.next ??
                    ({ kind: "Null" } as Cell);
                  return (
                    <div
                      key={col.name}
                      className="border-border/40 flex min-w-[8rem] items-baseline gap-1 rounded-sm border px-2 py-0.5"
                    >
                      <span className="text-muted-foreground text-[10px]">
                        {col.name}
                      </span>
                      <EditableCell
                        value={cellValue}
                        columnIndex={idx}
                        row={p.capturedRow}
                        meta={result.meta}
                        connId={connId}
                        rowKeyOverride={p.rowKey}
                      />
                    </div>
                  );
                })}
              </div>
              <button
                type="button"
                onClick={() => usePendingChanges.getState().revertRow(p.rowKey)}
                className="text-muted-foreground hover:text-destructive shrink-0 px-1 text-xs"
                title="Drop this insert"
              >
                ✕
              </button>
            </div>
          ))}
        </div>
      )}
      <table className="w-full border-collapse">
        <thead className="bg-muted/50 sticky top-0 z-10">
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id}>
              {hg.headers.map((h) => (
                <th
                  key={h.id}
                  onClick={h.column.getToggleSortingHandler()}
                  className="border-border cursor-pointer border-b px-3 py-1.5 text-left"
                >
                  {flexRender(h.column.columnDef.header, h.getContext())}
                  {h.column.getIsSorted() === "asc" && " ▲"}
                  {h.column.getIsSorted() === "desc" && " ▼"}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody
          style={{
            height: `${rowVirtualizer.getTotalSize()}px`,
            position: "relative",
            display: "block",
          }}
        >
          {rowVirtualizer.getVirtualItems().map((vi) => {
            const row = table.getRowModel().rows[vi.index];
            return (
              <tr
                key={row.id}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  display: "table",
                  tableLayout: "fixed",
                  width: "100%",
                  transform: `translateY(${vi.start}px)`,
                }}
              >
                {row.getVisibleCells().map((cell) => {
                  const columnIndex = result.columns.findIndex(
                    (c) => c.name === cell.column.id,
                  );
                  const rawRow = result.rows[cell.row.index];
                  const cellValue = rawRow[columnIndex];
                  return (
                    <td
                      key={cell.id}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        setMenu({
                          cell: cellValue,
                          columnIndex,
                          row: rawRow,
                          x: e.clientX,
                          y: e.clientY,
                        });
                      }}
                      className="border-border max-w-[24rem] truncate border-b px-3 py-1"
                    >
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext(),
                      )}
                    </td>
                  );
                })}
              </tr>
            );
          })}
        </tbody>
      </table>
      {menu && (
        <CellContextMenu
          cell={menu.cell}
          columnIndex={menu.columnIndex}
          row={menu.row}
          meta={result.meta}
          connId={connId}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          onFilter={onFilter}
        />
      )}
    </div>
  );
}
