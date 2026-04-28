import { useMemo, useRef } from "react";
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";

import type { Cell, QueryResult } from "@/lib/types";

import { renderCell } from "./cells";

type Row = Record<string, Cell>;

export function ResultsGrid({ result }: { result: QueryResult }) {
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
      result.columns.map((c) => ({
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
        cell: (info) => renderCell(info.getValue() as Cell),
      })),
    [result.columns],
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

  return (
    <div ref={parentRef} className="flex-1 overflow-auto font-mono text-xs">
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
                {row.getVisibleCells().map((cell) => (
                  <td
                    key={cell.id}
                    className="border-border max-w-[24rem] truncate border-b px-3 py-1"
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
