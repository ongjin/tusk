import type { IndexCandidate } from "@/lib/explain/planTypes";

interface Props {
  candidates: IndexCandidate[];
  onInsert: (sql: string) => void;
}

export function IndexCandidates({ candidates, onInsert }: Props) {
  if (candidates.length === 0) {
    return (
      <div className="text-muted-foreground p-3 text-xs">
        No verified index candidates. (No high-selectivity Seq Scan filters
        detected.)
      </div>
    );
  }
  return (
    <div className="grid grid-cols-1 gap-2 p-3 text-xs md:grid-cols-2">
      {candidates.map((c, i) => {
        const sql = `CREATE INDEX ON ${escIdent(c.schema)}.${escIdent(c.table)} (${c.columns.map(escIdent).join(", ")});`;
        return (
          <div
            key={`${c.schema}.${c.table}.${c.columns.join(",")}.${i}`}
            className={`bg-muted/30 rounded border p-2 ${
              c.verdict === "likely" ? "border-amber-500/60" : "border-border"
            }`}
          >
            <div className="mb-1 flex items-center justify-between">
              <span className="font-mono">
                {c.schema}.{c.table}({c.columns.join(", ")})
              </span>
              <span
                className={`rounded px-2 py-0.5 text-[10px] ${
                  c.verdict === "likely"
                    ? "bg-amber-500/30"
                    : "bg-muted text-muted-foreground"
                }`}
              >
                {c.verdict}
              </span>
            </div>
            <pre className="bg-background mb-1 overflow-x-auto rounded p-2 text-[11px]">
              {sql}
            </pre>
            <div className="text-muted-foreground mb-1 text-[11px]">
              {c.reason} · selectivity{" "}
              {c.selectivityEstimate !== null
                ? c.selectivityEstimate.toFixed(3)
                : "unknown"}
              {c.nDistinct !== null && ` · n_distinct=${c.nDistinct}`}
            </div>
            <button
              type="button"
              className="border-input hover:bg-accent rounded border px-2 py-0.5 text-[11px]"
              onClick={() => onInsert(sql)}
            >
              Insert into editor
            </button>
          </div>
        );
      })}
    </div>
  );
}

function escIdent(s: string): string {
  if (/^[a-z_][a-z0-9_]*$/.test(s)) return s;
  return `"${s.replace(/"/g, '""')}"`;
}
