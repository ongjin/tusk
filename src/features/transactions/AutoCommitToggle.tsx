import { toast } from "sonner";

import { useTransactions } from "@/store/transactions";

export function AutoCommitToggle({ connId }: { connId: string }) {
  const tx = useTransactions((s) => s.byConn[connId]);
  const begin = useTransactions((s) => s.begin);
  const rollback = useTransactions((s) => s.rollback);
  const active = tx?.active === true;

  const onToggle = async () => {
    try {
      if (active) {
        // toggling auto-commit ON while active = abort tx
        await rollback(connId);
        toast.warning("Transaction rolled back (auto-commit re-enabled)");
      } else {
        await begin(connId);
        toast.info("Auto-commit OFF — explicit transaction started");
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      toast.error(`Transaction error: ${msg}`);
    }
  };

  return (
    <button
      type="button"
      onClick={onToggle}
      className="border-input hover:bg-accent rounded border px-2 py-1 text-xs"
    >
      Auto-commit: {active ? "OFF" : "ON"}
    </button>
  );
}
