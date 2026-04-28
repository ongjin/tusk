import { useEffect, useState } from "react";

import { useHistory } from "@/store/history";

interface Props {
  onClose: () => void;
  onPick: (sql: string) => void;
}

export function HistoryPalette({ onClose, onPick }: Props) {
  const [q, setQ] = useState("");
  const entries = useHistory((s) => s.entries);
  const search = useHistory((s) => s.search);

  useEffect(() => {
    const t = setTimeout(() => {
      void search(q);
    }, 120);
    return () => clearTimeout(t);
  }, [q, search]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 pt-24"
      onClick={onClose}
    >
      <div
        className="bg-card w-[640px] rounded-sm border p-3 shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          autoFocus
          placeholder="Search history…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
          }}
          className="bg-background border-input w-full rounded-sm border px-2 py-1 text-xs"
        />
        <ul className="mt-2 max-h-[60vh] overflow-auto">
          {entries.length === 0 && (
            <li className="text-muted-foreground py-2 text-xs italic">
              {q ? "No matches" : "Type to search history"}
            </li>
          )}
          {entries.map((e) => (
            <li
              key={e.id}
              className="hover:bg-muted cursor-pointer rounded-sm px-2 py-1 text-xs"
              onClick={() => onPick(e.sqlFull ?? e.sqlPreview)}
            >
              <span className="text-muted-foreground mr-2 font-mono">
                {new Date(e.startedAt).toISOString().slice(0, 19)}
              </span>
              {e.source === "ai" ? (
                <span className="font-mono">
                  <span aria-hidden>✦</span> AI: {e.sqlPreview}
                </span>
              ) : (
                <span className="font-mono">{e.sqlPreview}</span>
              )}
              {e.statementCount > 1 && (
                <span className="ml-2 text-amber-500">
                  (tx · {e.statementCount} stmts)
                </span>
              )}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
