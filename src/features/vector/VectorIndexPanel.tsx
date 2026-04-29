import { useEffect, useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Input } from "@/components/ui/input";
import { listVectorIndexes } from "@/lib/tauri";
import type { VectorIndex } from "@/lib/vector/types";
import { useTabs } from "@/store/tabs";
import { useVectorMeta } from "@/store/useVectorMeta";

export interface VectorIndexPanelOpen {
  connId: string;
  schema: string;
  table: string;
}

interface Props {
  open: VectorIndexPanelOpen | null;
  onClose: () => void;
}

export function VectorIndexPanel({ open, onClose }: Props) {
  const [indexes, setIndexes] = useState<VectorIndex[]>([]);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const vectorCols = useVectorMeta((s) =>
    open ? s.vectorColumnsForTable(open.connId, open.schema, open.table) : [],
  );

  useEffect(() => {
    if (!open) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setLoading(true);
    setErr(null);
    listVectorIndexes(open.connId, open.schema, open.table)
      .then(setIndexes)
      .catch((e) => setErr(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }, [open]);

  return (
    <Sheet open={!!open} onOpenChange={(o) => !o && onClose()}>
      <SheetContent side="right" className="w-[640px] max-w-[90vw]">
        <SheetHeader>
          <SheetTitle>
            Vector indexes — {open?.schema}.{open?.table}
          </SheetTitle>
        </SheetHeader>
        <div className="mt-4 flex flex-col gap-4 text-xs">
          {loading && <div className="text-muted-foreground">Loading…</div>}
          {err && <div className="text-red-500">Error: {err}</div>}
          {!loading && !err && indexes.length === 0 && (
            <div className="text-muted-foreground">
              No HNSW or IVFFlat indexes on this table.
            </div>
          )}
          {indexes.length > 0 && (
            <table className="w-full">
              <thead>
                <tr className="text-muted-foreground">
                  <th className="text-left">Name</th>
                  <th className="text-left">Column</th>
                  <th className="text-left">Method</th>
                  <th className="text-left">Params</th>
                  <th className="text-left">Size</th>
                </tr>
              </thead>
              <tbody>
                {indexes.map((i) => (
                  <tr key={i.name} className="border-border border-t">
                    <td className="font-mono">{i.name}</td>
                    <td>{i.column}</td>
                    <td>{i.method}</td>
                    <td>{paramsLabel(i)}</td>
                    <td>{(i.sizeBytes / 1024).toFixed(0)} KB</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <CreateIndexForm
            schema={open?.schema ?? ""}
            table={open?.table ?? ""}
            vectorCols={vectorCols.map((c) => c.column)}
          />
        </div>
      </SheetContent>
    </Sheet>
  );
}

function paramsLabel(i: VectorIndex): string {
  const p = i.params;
  const parts: string[] = [];
  if (i.method === "hnsw") {
    parts.push(`m=${p.m ?? "?"}`);
    parts.push(`ef_construction=${p.efConstruction ?? "?"}`);
  } else {
    parts.push(`lists=${p.lists ?? "?"}`);
  }
  if (p.ops) parts.push(p.ops);
  return parts.join(" · ");
}

function CreateIndexForm({
  schema,
  table,
  vectorCols,
}: {
  schema: string;
  table: string;
  vectorCols: string[];
}) {
  const [col, setCol] = useState(vectorCols[0] ?? "");
  const [method, setMethod] = useState<"hnsw" | "ivfflat">("hnsw");
  const [m, setM] = useState(16);
  const [ef, setEf] = useState(64);
  const [lists, setLists] = useState(100);
  const [ops, setOps] = useState("vector_cosine_ops");

  const sql = useMemo(() => {
    if (!col) return "";
    if (method === "hnsw") {
      return `CREATE INDEX ON ${esc(schema)}.${esc(table)} USING hnsw (${esc(col)} ${ops}) WITH (m=${m}, ef_construction=${ef});`;
    }
    return `CREATE INDEX ON ${esc(schema)}.${esc(table)} USING ivfflat (${esc(col)} ${ops}) WITH (lists=${lists});`;
  }, [col, method, m, ef, lists, ops, schema, table]);

  return (
    <div className="border-border border-t pt-3">
      <h4 className="mb-2 font-semibold">Create index</h4>
      <div className="grid grid-cols-[120px_1fr] items-center gap-2">
        <label className="text-muted-foreground">Column</label>
        <select
          value={col}
          onChange={(e) => setCol(e.target.value)}
          className="border-input rounded border bg-transparent px-2 py-1 text-xs"
        >
          {vectorCols.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
        <label className="text-muted-foreground">Method</label>
        <select
          value={method}
          onChange={(e) => setMethod(e.target.value as "hnsw" | "ivfflat")}
          className="border-input rounded border bg-transparent px-2 py-1 text-xs"
        >
          <option value="hnsw">HNSW</option>
          <option value="ivfflat">IVFFlat</option>
        </select>
        <label className="text-muted-foreground">Operator class</label>
        <select
          value={ops}
          onChange={(e) => setOps(e.target.value)}
          className="border-input rounded border bg-transparent px-2 py-1 text-xs"
        >
          <option value="vector_cosine_ops">vector_cosine_ops</option>
          <option value="vector_l2_ops">vector_l2_ops</option>
          <option value="vector_ip_ops">vector_ip_ops</option>
        </select>
        {method === "hnsw" ? (
          <>
            <label className="text-muted-foreground">m</label>
            <Input
              type="number"
              value={m}
              onChange={(e) => setM(Number(e.target.value))}
            />
            <label className="text-muted-foreground">ef_construction</label>
            <Input
              type="number"
              value={ef}
              onChange={(e) => setEf(Number(e.target.value))}
            />
          </>
        ) : (
          <>
            <label className="text-muted-foreground">lists</label>
            <Input
              type="number"
              value={lists}
              onChange={(e) => setLists(Number(e.target.value))}
            />
          </>
        )}
      </div>
      <pre className="bg-muted mt-2 overflow-x-auto rounded p-2 text-[11px]">
        {sql}
      </pre>
      <Button
        size="sm"
        className="mt-2"
        onClick={() => {
          const t = useTabs.getState();
          const tab = t.tabs.find((x) => x.id === t.activeId);
          if (!tab) return;
          const next =
            (tab.sql ?? "") +
            (tab.sql.endsWith("\n") || tab.sql === "" ? "" : "\n") +
            sql +
            "\n";
          t.updateSql(tab.id, next);
        }}
      >
        Insert into editor
      </Button>
    </div>
  );
}

function esc(s: string): string {
  return `"${s.replace(/"/g, '""')}"`;
}
