import type { QueryResult } from "@/lib/types";

interface Props {
  result?: QueryResult;
  error?: string;
  busy?: boolean;
}

export function ResultsHeader({ result, error, busy }: Props) {
  return (
    <div className="border-border bg-muted/40 flex items-center gap-3 border-b px-3 py-1.5 text-xs">
      {busy && <span className="text-muted-foreground">Running…</span>}
      {!busy && error && <span className="text-destructive">{error}</span>}
      {!busy && result && (
        <>
          <span>{result.rowCount} rows</span>
          <span className="text-muted-foreground">·</span>
          <span>{result.durationMs} ms</span>
          <span className="text-muted-foreground">·</span>
          {result.meta.editable ? (
            <span
              title={`Editable — ${result.meta.table?.schema}.${result.meta.table?.name}`}
              className="text-xs text-amber-500"
            >
              ✏️
            </span>
          ) : (
            <span
              title={`Read-only — ${result.meta.reason ?? "unknown"}`}
              className="text-muted-foreground text-xs"
            >
              🔒
            </span>
          )}
        </>
      )}
      {!busy && !result && !error && (
        <span className="text-muted-foreground">
          No result yet — Cmd+Enter to run.
        </span>
      )}
    </div>
  );
}
