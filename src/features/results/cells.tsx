import { useState } from "react";

export function Cell({ value, type }: { value: unknown; type: string }) {
  if (value === null || value === undefined) {
    return <span className="text-muted-foreground italic">NULL</span>;
  }
  if (type === "json" || type === "jsonb" || typeof value === "object") {
    return <JsonCell value={value} />;
  }
  if (typeof value === "boolean") {
    return <span>{value ? "true" : "false"}</span>;
  }
  return <span className="font-mono">{String(value)}</span>;
}

function JsonCell({ value }: { value: unknown }) {
  const [open, setOpen] = useState(false);
  const text = JSON.stringify(value);
  const truncated = text.length > 80 ? `${text.slice(0, 77)}…` : text;
  return (
    <button
      onClick={() => setOpen((o) => !o)}
      className="text-left font-mono"
      title={open ? "click to collapse" : "click to expand"}
    >
      {open ? (
        <pre className="text-xs whitespace-pre-wrap">
          {JSON.stringify(value, null, 2)}
        </pre>
      ) : (
        truncated
      )}
    </button>
  );
}
