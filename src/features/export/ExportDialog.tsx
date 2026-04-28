import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

import type { Cell, ResultMeta } from "@/lib/types";

interface Props {
  rows: Cell[][];
  meta: ResultMeta;
  onClose: () => void;
}

type Format = "Csv" | "Json" | "SqlInsert";
type Scope = "all" | "selected";

export function ExportDialog({ rows, meta, onClose }: Props) {
  const [format, setFormat] = useState<Format>("Csv");
  const [bom, setBom] = useState(false);
  const [scope, setScope] = useState<Scope>("all");
  const [busy, setBusy] = useState(false);
  const sqlReady = format !== "SqlInsert" || !!meta.table;

  const run = async () => {
    if (!sqlReady) return;
    setBusy(true);
    try {
      const path = await save({
        defaultPath: meta.table?.name ?? "result",
        filters: [
          {
            name: format,
            extensions: [
              format === "Csv" ? "csv" : format === "Json" ? "json" : "sql",
            ],
          },
        ],
      });
      if (!path) {
        setBusy(false);
        return;
      }
      const cols = meta.columnTypes.map((c) => c.name);
      const useRows = scope === "all" ? rows : [];
      await invoke("export_result", {
        req: {
          format,
          path,
          columns: cols,
          rows: useRows,
          includeBom: bom,
          table:
            format === "SqlInsert" && meta.table
              ? `"${meta.table.schema}"."${meta.table.name}"`
              : null,
        },
      });
      onClose();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
      onClick={onClose}
    >
      <div
        className="bg-card w-[400px] rounded-sm border p-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-sm font-medium">Export result</h2>
        <div className="mt-3 space-y-2 text-xs">
          <label className="flex items-center gap-2">
            Format
            <select
              value={format}
              onChange={(e) => setFormat(e.target.value as Format)}
              className="bg-background border-input rounded-sm border px-2 py-0.5"
            >
              <option value="Csv">CSV</option>
              <option value="Json">JSON</option>
              <option value="SqlInsert">SQL INSERT</option>
            </select>
          </label>
          {format === "Csv" && (
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={bom}
                onChange={(e) => setBom(e.target.checked)}
              />
              UTF-8 BOM
            </label>
          )}
          <label className="flex items-center gap-2">
            Scope
            <select
              value={scope}
              onChange={(e) => setScope(e.target.value as Scope)}
              className="bg-background border-input rounded-sm border px-2 py-0.5"
            >
              <option value="all">All rows ({rows.length})</option>
              <option value="selected">Selected rows</option>
            </select>
          </label>
          <p className="text-muted-foreground italic">
            Note: row selection is a v1.5 feature; &ldquo;Selected&rdquo;
            exports nothing today.
          </p>
          {format === "SqlInsert" && !meta.table && (
            <p className="text-red-500">
              SQL INSERT requires a single-table editable result.
            </p>
          )}
        </div>
        <div className="mt-4 flex justify-end gap-2 text-xs">
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            className="rounded-sm border px-2 py-1"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={run}
            disabled={busy || !sqlReady}
            className="rounded-sm border bg-amber-500 px-2 py-1 text-black disabled:opacity-50"
          >
            {busy ? "Exporting…" : "Export"}
          </button>
        </div>
      </div>
    </div>
  );
}
