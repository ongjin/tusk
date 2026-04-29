import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { executeQuery, sampleVectors } from "@/lib/tauri";
import type { QueryResult } from "@/lib/types";
import { runUmap } from "@/lib/vector/umapWorker";
import { useTabs, type UmapTabState } from "@/store/tabs";

import { UmapControls } from "./UmapControls";
import { UmapScatter } from "./UmapScatter";

export function UmapTab({ tabId }: { tabId: string }) {
  const tab = useTabs((s) => s.tabs.find((t) => t.id === tabId));
  const patchUmap = useTabs((s) => s.patchUmap);
  const u = tab?.umap;
  const [rowDetail, setRowDetail] = useState<Record<string, unknown> | null>(
    null,
  );

  useEffect(() => {
    if (!u) return;
    if (u.status === "loading-pk") {
      void resolvePkAndStart(tabId, u, patchUmap);
    } else if (u.status === "sampling") {
      void runPipeline(tabId, u, patchUmap);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [u?.status]);

  const points = useMemo(() => u?.points ?? [], [u?.points]);

  if (!tab || !u) return null;

  return (
    <div className="grid h-full grid-cols-[260px_1fr_320px]">
      <UmapControls
        sample={u.sample}
        nNeighbors={u.nNeighbors}
        minDist={u.minDist}
        running={u.status === "sampling" || u.status === "computing"}
        onChange={(p) => patchUmap(tabId, p)}
        onRun={() =>
          patchUmap(tabId, {
            status: "sampling",
            progress: 0,
            error: undefined,
          })
        }
      />
      <div className="relative">
        {(u.status === "sampling" || u.status === "computing") && (
          <div className="text-muted-foreground absolute right-3 top-3 z-10 text-xs">
            {u.status} {Math.round(u.progress * 100)}%
          </div>
        )}
        {u.status === "error" && (
          <div className="absolute inset-0 flex items-center justify-center text-sm text-red-500">
            {u.error ?? "UMAP failed"}
          </div>
        )}
        <UmapScatter
          points={points}
          selectedIdx={u.selectedIdx}
          onSelect={(idx) => {
            patchUmap(tabId, { selectedIdx: idx });
            void fetchRow(u, points[idx]).then(setRowDetail);
          }}
        />
      </div>
      <div className="border-border overflow-auto border-l p-3 text-xs">
        {rowDetail ? (
          <pre className="whitespace-pre-wrap">
            {JSON.stringify(rowDetail, null, 2)}
          </pre>
        ) : (
          <div className="text-muted-foreground">Click a point to inspect.</div>
        )}
      </div>
    </div>
  );
}

async function resolvePkAndStart(
  tabId: string,
  u: UmapTabState,
  patch: (id: string, p: Partial<UmapTabState>) => void,
): Promise<void> {
  // list_columns doesn't return PK info, so probe via a zero-row select and
  // read the populated pkColumns from result.meta (server fills via fetch_table_meta).
  try {
    const sql = `SELECT * FROM ${escIdent(u.schema)}.${escIdent(u.table)} LIMIT 0`;
    const result = await executeQuery(u.connId, sql);
    const pkCols = result.meta.pkColumns ?? [];
    if (pkCols.length === 0) {
      patch(tabId, {
        status: "error",
        error:
          "Table has no primary key — UMAP needs PK to map points back to rows.",
      });
      return;
    }
    patch(tabId, { pkCols, status: "sampling", progress: 0 });
  } catch (e) {
    patch(tabId, {
      status: "error",
      error: e instanceof Error ? e.message : String(e),
    });
  }
}

async function runPipeline(
  tabId: string,
  u: UmapTabState,
  patch: (id: string, p: Partial<UmapTabState>) => void,
): Promise<void> {
  try {
    const sampled = await sampleVectors({
      connectionId: u.connId,
      schema: u.schema,
      table: u.table,
      vecCol: u.vecCol,
      pkCols: u.pkCols,
      limit: u.sample,
    });
    if (sampled.rows.length === 0) {
      patch(tabId, { status: "error", error: "No vectors sampled" });
      return;
    }
    const dim = sampled.rows[0].vec.length;
    const count = sampled.rows.length;
    const flat = new Float32Array(count * dim);
    for (let i = 0; i < count; i++) flat.set(sampled.rows[i].vec, i * dim);
    patch(tabId, { status: "computing", progress: 0 });
    const coords = await runUmap({
      vecs: flat,
      dim,
      count,
      nNeighbors: u.nNeighbors,
      minDist: u.minDist,
      onProgress: (v) => patch(tabId, { progress: v }),
    });
    const points = sampled.rows.map((r, i) => ({
      x: coords[i * 2],
      y: coords[i * 2 + 1],
      pkJson: r.pkJson,
    }));
    patch(tabId, { status: "ready", progress: 1, points });
  } catch (e) {
    patch(tabId, {
      status: "error",
      error: e instanceof Error ? e.message : String(e),
    });
    toast.error(e instanceof Error ? e.message : String(e));
  }
}

async function fetchRow(
  u: UmapTabState,
  point: { pkJson: Record<string, unknown> },
): Promise<Record<string, unknown> | null> {
  // execute_query_with_params doesn't exist — use literal interpolation via execute_query.
  const lit = u.pkCols
    .map((c) => `${escIdent(c)} = ${litVal(point.pkJson[c])}`)
    .join(" AND ");
  const sql = `SELECT * FROM ${escIdent(u.schema)}.${escIdent(u.table)} WHERE ${lit} LIMIT 1`;
  try {
    const r: QueryResult = await executeQuery(u.connId, sql);
    if (!r.rows || r.rows.length === 0) return null;
    return r.rows[0] as unknown as Record<string, unknown>;
  } catch {
    return null;
  }
}

function escIdent(s: string): string {
  return `"${s.replace(/"/g, '""')}"`;
}

function litVal(v: unknown): string {
  if (typeof v === "number") return String(v);
  if (typeof v === "boolean") return v ? "TRUE" : "FALSE";
  if (v === null || v === undefined) return "NULL";
  return `'${String(v).replace(/'/g, "''")}'`;
}
