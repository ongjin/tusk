import { useCallback, useEffect, useState } from "react";
import { Dialog as DialogPrimitive } from "radix-ui";

interface ConfirmRequest {
  title: string;
  body: string;
  buttons: string[];
  resolve: (choice: string | null) => void;
}

let pending: ConfirmRequest | null = null;
let pendingListener: ((req: ConfirmRequest | null) => void) | null = null;

export function openConfirmModal(opts: {
  title: string;
  body: string;
  buttons: string[];
}): Promise<string | null> {
  return new Promise((resolve) => {
    if (pending) {
      pending.resolve(null);
    }
    pending = { ...opts, resolve };
    pendingListener?.(pending);
  });
}

export function ConfirmModalHost() {
  // Initial state captures any request queued before mount; subsequent
  // requests come through the listener registered in the effect.
  const [req, setReq] = useState<ConfirmRequest | null>(() => pending);

  useEffect(() => {
    pendingListener = setReq;
    return () => {
      pendingListener = null;
    };
  }, []);

  const close = useCallback(
    (choice: string | null) => {
      if (req) {
        req.resolve(choice);
        pending = null;
        setReq(null);
      }
    },
    [req],
  );

  if (!req) return null;
  return (
    <DialogPrimitive.Root
      open
      onOpenChange={(open) => {
        if (!open) close(null);
      }}
    >
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <DialogPrimitive.Content className="bg-card fixed top-1/2 left-1/2 z-50 w-[420px] -translate-x-1/2 -translate-y-1/2 rounded border p-4 shadow">
          <DialogPrimitive.Title className="text-sm font-medium">
            {req.title}
          </DialogPrimitive.Title>
          <DialogPrimitive.Description className="text-muted-foreground mt-2 text-xs">
            {req.body}
          </DialogPrimitive.Description>
          <div className="mt-4 flex justify-end gap-2 text-xs">
            {req.buttons.map((b) => (
              <button
                key={b}
                type="button"
                onClick={() => close(b)}
                className="border-input hover:bg-accent rounded border px-2 py-1"
              >
                {b}
              </button>
            ))}
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
