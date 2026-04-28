import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { HistoryEntry, HistoryStatement } from "@/lib/types";
import { useTransactions } from "@/store/transactions";

export function TxSidePanel({ connId }: { connId: string }) {
  const tx = useTransactions((s) => s.byConn[connId]);
  const [stmts, setStmts] = useState<HistoryStatement[]>([]);
  const txId = tx?.active ? tx.txId : undefined;

  useEffect(() => {
    if (!txId) return;
    let cancelled = false;
    const tick = async () => {
      try {
        // Re-fetch list every 1s while tx is active.
        const entryId = await getEntryIdForTx(connId, txId);
        if (!entryId || cancelled) return;
        const list = await invoke<HistoryStatement[]>(
          "list_history_statements",
          { entryId },
        );
        if (!cancelled) setStmts(list);
      } catch {
        // Swallow: side panel is best-effort while tx runs.
      }
    };
    void tick();
    const id = setInterval(tick, 1000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [connId, txId]);

  // Clear stale statements when the tx ends (or txId changes between sessions).
  // This is a derived behavior, not synced via setState in an effect.
  const visible = tx?.active ? stmts : [];

  if (!tx?.active) return null;
  return (
    <aside className="border-border w-64 shrink-0 overflow-y-auto border-l p-2 text-xs">
      <h3 className="font-medium">Transaction statements</h3>
      {visible.length === 0 ? (
        <p className="text-muted-foreground mt-2">No statements yet.</p>
      ) : (
        <ol className="mt-2 space-y-1">
          {visible.map((s) => (
            <li key={s.id} className="truncate" title={s.sql}>
              {s.ordinal + 1}. {s.sql.slice(0, 60)}
            </li>
          ))}
        </ol>
      )}
    </aside>
  );
}

async function getEntryIdForTx(
  connId: string,
  txId: string,
): Promise<string | null> {
  // Look up the most-recent entry whose tx_id matches.
  const entries = await invoke<HistoryEntry[]>("list_history", {
    connectionId: connId,
    query: null,
    limit: 50,
  });
  return entries.find((e) => e.txId === txId)?.id ?? null;
}
