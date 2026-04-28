import { useEffect, useMemo, useState } from "react";
import { Dialog as DialogPrimitive } from "radix-ui";
import { AlertTriangle } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { DestructiveFinding } from "@/lib/types";

let nextId = 0;

interface PendingRequest {
  id: number;
  findings: DestructiveFinding[];
  sql: string;
  strict: boolean;
  resolve: (run: boolean) => void;
}

let pending: PendingRequest | null = null;
let listener: ((r: PendingRequest | null) => void) | null = null;

export function confirmDestructive(opts: {
  findings: DestructiveFinding[];
  sql: string;
  strict: boolean;
}): Promise<boolean> {
  return new Promise((resolve) => {
    if (pending) pending.resolve(false);
    pending = { id: ++nextId, ...opts, resolve };
    listener?.(pending);
  });
}

// Inner dialog — receives a stable req object and owns the `typed` state.
// Rendered with a unique key per request so `typed` resets automatically.
function DestructiveDialog({
  req,
  onClose,
}: {
  req: PendingRequest;
  onClose: (run: boolean) => void;
}) {
  const [typed, setTyped] = useState("");

  const requiredKeyword = useMemo(() => {
    const candidates = [
      "DROP",
      "TRUNCATE",
      "DELETE",
      "UPDATE",
      "ALTER",
      "GRANT",
      "REVOKE",
      "VACUUM",
    ];
    return (
      candidates.find((c) => req.sql.toUpperCase().includes(c)) ?? "CONFIRM"
    );
  }, [req.sql]);

  const canRun = !req.strict || typed.trim().toUpperCase() === requiredKeyword;

  return (
    <DialogPrimitive.Root open onOpenChange={(o) => !o && onClose(false)}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <DialogPrimitive.Content
          role="alertdialog"
          className="bg-card fixed top-1/2 left-1/2 z-50 w-[560px] -translate-x-1/2 -translate-y-1/2 rounded border p-4 shadow"
        >
          <DialogPrimitive.Title className="flex items-center gap-2 text-sm font-medium text-amber-700">
            <AlertTriangle className="h-4 w-4" />
            Confirm destructive operations
          </DialogPrimitive.Title>
          <ul className="mt-3 space-y-1 text-xs">
            {req.findings.map((f, i) => (
              <li key={i}>
                <span className="font-mono">{f.kind}</span> — {f.message}
              </li>
            ))}
          </ul>
          <pre className="bg-muted mt-3 max-h-40 overflow-auto rounded p-2 text-xs">
            {req.sql}
          </pre>
          {req.strict && (
            <label className="mt-3 block text-xs">
              Type <code className="font-mono">{requiredKeyword}</code> to
              confirm:
              <input
                value={typed}
                onChange={(e) => setTyped(e.target.value)}
                className="border-input mt-1 w-full rounded border px-2 py-1"
                autoFocus
              />
            </label>
          )}
          <div className="mt-4 flex justify-end gap-2">
            <Button variant="ghost" onClick={() => onClose(false)}>
              Cancel
            </Button>
            <Button
              disabled={!canRun}
              onClick={() => onClose(true)}
              className={req.strict ? "" : "bg-amber-600 hover:bg-amber-500"}
            >
              {req.strict ? "Run" : "Run anyway"}
            </Button>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}

export function DestructiveModalHost() {
  const [req, setReq] = useState<PendingRequest | null>(() => pending);

  useEffect(() => {
    listener = setReq;
    return () => {
      listener = null;
    };
  }, []);

  const close = (run: boolean) => {
    if (req) {
      req.resolve(run);
      pending = null;
      setReq(null);
    }
  };

  if (!req) return null;

  // Key on req.id (a monotonic nonce) so DestructiveDialog remounts (and typed resets)
  // each time a new request arrives, even if sql/strict are identical.
  return <DestructiveDialog key={req.id} req={req} onClose={close} />;
}
