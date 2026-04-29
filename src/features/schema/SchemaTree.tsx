import { useCallback, useEffect, useRef, useState } from "react";

import { ScrollArea } from "@/components/ui/scroll-area";
import { useConnections } from "@/store/connections";
import { useSchema } from "@/store/schema";
import { useVectorActions } from "@/store/useVectorActions";
import { useVectorMeta } from "@/store/useVectorMeta";

import { SchemaNode } from "./SchemaNode";

export function SchemaTree() {
  const items = useConnections((s) => s.items);
  const connected = items.filter((i) => i.connected);

  if (connected.length === 0) {
    return (
      <p className="text-muted-foreground px-3 py-2 text-xs">
        Connect to a database to browse its schema.
      </p>
    );
  }

  return (
    <ScrollArea className="flex-1">
      <div className="flex flex-col gap-1 p-2">
        {connected.map((c) => (
          <ConnectionBranch key={c.id} connectionId={c.id} name={c.name} />
        ))}
      </div>
    </ScrollArea>
  );
}

function ConnectionBranch({
  connectionId,
  name,
}: {
  connectionId: string;
  name: string;
}) {
  const schemas = useSchema((s) => s.schemas[connectionId]);
  const load = useSchema((s) => s.loadSchemas);

  const onExpand = useCallback(() => {
    load(connectionId);
  }, [load, connectionId]);

  return (
    <SchemaNode label={name} onExpand={onExpand}>
      {schemas?.state === "loading" && <Hint>loading…</Hint>}
      {schemas?.state === "error" && <Hint>{schemas.error}</Hint>}
      {schemas?.state === "ready" &&
        schemas.data!.map((schema) => (
          <SchemaBranch
            key={schema}
            connectionId={connectionId}
            schema={schema}
            indent={1}
          />
        ))}
    </SchemaNode>
  );
}

function SchemaBranch({
  connectionId,
  schema,
  indent,
}: {
  connectionId: string;
  schema: string;
  indent: number;
}) {
  const key = `${connectionId}:${schema}`;
  const tables = useSchema((s) => s.tables[key]);
  const loadTables = useSchema((s) => s.loadTables);

  const onExpand = useCallback(() => {
    loadTables(connectionId, schema);
  }, [loadTables, connectionId, schema]);

  return (
    <SchemaNode label={schema} indent={indent} onExpand={onExpand}>
      {tables?.state === "loading" && <Hint indent={indent + 1}>loading…</Hint>}
      {tables?.state === "error" && (
        <Hint indent={indent + 1}>{tables.error}</Hint>
      )}
      {tables?.state === "ready" &&
        tables.data!.map((table) => (
          <TableBranch
            key={table}
            connectionId={connectionId}
            schema={schema}
            table={table}
            indent={indent + 1}
          />
        ))}
    </SchemaNode>
  );
}

type ColumnMenuState = {
  x: number;
  y: number;
  connId: string;
  schema: string;
  table: string;
  column: string;
  dim: number;
};

type TableMenuState = {
  x: number;
  y: number;
  connId: string;
  schema: string;
  table: string;
};

function TableBranch({
  connectionId,
  schema,
  table,
  indent,
}: {
  connectionId: string;
  schema: string;
  table: string;
  indent: number;
}) {
  const key = `${connectionId}:${schema}:${table}`;
  const cols = useSchema((s) => s.columns[key]);
  const loadColumns = useSchema((s) => s.loadColumns);
  const hasVectorAt = useVectorMeta((s) => s.hasVectorAt);
  const tableHasVector = useVectorMeta((s) => s.tableHasVector);

  const [columnMenu, setColumnMenu] = useState<ColumnMenuState | null>(null);
  const [tableMenu, setTableMenu] = useState<TableMenuState | null>(null);

  const onExpand = useCallback(() => {
    loadColumns(connectionId, schema, table);
  }, [loadColumns, connectionId, schema, table]);

  return (
    <>
      <SchemaNode
        label={table}
        indent={indent}
        onExpand={onExpand}
        onContextMenu={(e) => {
          if (!tableHasVector(connectionId, schema, table)) return;
          e.preventDefault();
          setTableMenu({
            x: e.clientX,
            y: e.clientY,
            connId: connectionId,
            schema,
            table,
          });
        }}
      >
        {cols?.state === "loading" && <Hint indent={indent + 1}>loading…</Hint>}
        {cols?.state === "error" && (
          <Hint indent={indent + 1}>{cols.error}</Hint>
        )}
        {cols?.state === "ready" &&
          cols.data!.map((c) => (
            <div
              key={c.name}
              className="text-muted-foreground flex justify-between text-xs"
              style={{ paddingLeft: 4 + (indent + 1) * 12, paddingRight: 8 }}
              onContextMenu={(e) => {
                const v = hasVectorAt(connectionId, schema, table, c.name);
                if (!v) return; // let default browser menu happen
                e.preventDefault();
                setColumnMenu({
                  x: e.clientX,
                  y: e.clientY,
                  connId: connectionId,
                  schema,
                  table,
                  column: c.name,
                  dim: v.dim,
                });
              }}
            >
              <span className="flex items-center">
                <span>{c.name}</span>
                {(() => {
                  const v = hasVectorAt(connectionId, schema, table, c.name);
                  if (!v) return null;
                  return (
                    <>
                      <span
                        className="text-muted-foreground ml-2 rounded bg-blue-500/10 px-1 text-[10px]"
                        title={`vector(${v.dim})`}
                      >
                        vec({v.dim})
                      </span>
                      {!v.hasIndex && (
                        <span
                          className="ml-1 text-amber-600"
                          title="No HNSW/IVFFlat index — sequential scan only"
                        >
                          ⚠
                        </span>
                      )}
                    </>
                  );
                })()}
              </span>
              <span>
                {c.data_type}
                {!c.is_nullable && " ·"}
                {!c.is_nullable && (
                  <span className="text-foreground"> NOT NULL</span>
                )}
              </span>
            </div>
          ))}
      </SchemaNode>
      {columnMenu && (
        <VectorContextMenu
          x={columnMenu.x}
          y={columnMenu.y}
          onClose={() => setColumnMenu(null)}
          items={[
            {
              label: "Visualize (UMAP)",
              onSelect: () => {
                const open = useVectorActions.getState().openUmap;
                if (open)
                  open({
                    connId: columnMenu.connId,
                    schema: columnMenu.schema,
                    table: columnMenu.table,
                    vecCol: columnMenu.column,
                    pkCols: [], // pkCols resolved by UmapTab if empty
                    dim: columnMenu.dim,
                  });
              },
            },
          ]}
        />
      )}
      {tableMenu && (
        <VectorContextMenu
          x={tableMenu.x}
          y={tableMenu.y}
          onClose={() => setTableMenu(null)}
          items={[
            {
              label: "Vector indexes",
              onSelect: () => {
                const open = useVectorActions.getState().openIndexPanel;
                if (open)
                  open({
                    connId: tableMenu.connId,
                    schema: tableMenu.schema,
                    table: tableMenu.table,
                  });
              },
            },
          ]}
        />
      )}
    </>
  );
}

function Hint({
  children,
  indent = 0,
}: {
  children: React.ReactNode;
  indent?: number;
}) {
  return (
    <p
      className="text-muted-foreground text-xs italic"
      style={{ paddingLeft: 4 + indent * 12 }}
    >
      {children}
    </p>
  );
}

function VectorContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: { label: string; onSelect: () => void }[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

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

  return (
    <div
      ref={ref}
      style={{ left: x, top: y }}
      className="bg-card fixed z-50 min-w-[10rem] rounded-sm border text-xs shadow-md"
    >
      {items.map((item) => (
        <button
          key={item.label}
          type="button"
          onClick={() => {
            item.onSelect();
            onClose();
          }}
          className="hover:bg-muted block w-full px-3 py-1 text-left text-xs"
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
