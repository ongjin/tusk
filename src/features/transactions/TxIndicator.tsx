import { useEffect, useState } from "react";

import { useTransactions } from "@/store/transactions";

export function TxIndicator({ connId }: { connId: string }) {
  const tx = useTransactions((s) => s.byConn[connId]);
  const commit = useTransactions((s) => s.commit);
  const rollback = useTransactions((s) => s.rollback);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!tx?.active) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [tx?.active]);

  if (!tx?.active) return null;

  const since = tx.startedAt
    ? `${Math.floor((now - tx.startedAt) / 1000)}s`
    : "";

  return (
    <div className="flex items-center gap-2 text-xs">
      <span
        aria-hidden
        className="inline-block size-2 rounded-full bg-amber-500"
      />
      <span className="text-muted-foreground">
        Transaction · {tx.statementCount} stmts · {since}
      </span>
      <button
        type="button"
        className="border-input hover:bg-accent rounded border px-2 py-0.5"
        onClick={() => void commit(connId)}
      >
        Commit
      </button>
      <button
        type="button"
        className="border-input hover:bg-accent rounded border px-2 py-0.5"
        onClick={() => void rollback(connId)}
      >
        Rollback
      </button>
    </div>
  );
}
