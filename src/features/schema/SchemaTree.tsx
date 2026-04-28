import { useCallback } from "react";

import { ScrollArea } from "@/components/ui/scroll-area";
import { useConnections } from "@/store/connections";
import { useSchema } from "@/store/schema";

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

  const onExpand = useCallback(() => {
    loadColumns(connectionId, schema, table);
  }, [loadColumns, connectionId, schema, table]);

  return (
    <SchemaNode label={table} indent={indent} onExpand={onExpand}>
      {cols?.state === "loading" && <Hint indent={indent + 1}>loading…</Hint>}
      {cols?.state === "error" && <Hint indent={indent + 1}>{cols.error}</Hint>}
      {cols?.state === "ready" &&
        cols.data!.map((c) => (
          <div
            key={c.name}
            className="text-muted-foreground flex justify-between text-xs"
            style={{ paddingLeft: 4 + (indent + 1) * 12, paddingRight: 8 }}
          >
            <span>{c.name}</span>
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
