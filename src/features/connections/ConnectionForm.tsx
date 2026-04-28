// src/features/connections/ConnectionForm.tsx
import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useConnections } from "@/store/connections";
import type { NewConnection } from "@/lib/types";

const EMPTY: NewConnection = {
  name: "",
  host: "127.0.0.1",
  port: 5432,
  dbUser: "postgres",
  database: "postgres",
  sslMode: "prefer",
  sshKind: "None",
  sshAlias: null,
  sshHost: null,
  sshPort: null,
  sshUser: null,
  sshKeyPath: null,
};

export function ConnectionForm() {
  const add = useConnections((s) => s.add);
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<NewConnection>(EMPTY);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);

  function reset() {
    setDraft(EMPTY);
    setPassword("");
  }

  async function onSave() {
    if (!draft.name.trim()) {
      toast.error("Name is required");
      return;
    }
    setBusy(true);
    try {
      await add(draft, password);
      toast.success(`Saved "${draft.name}"`);
      reset();
      setOpen(false);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to save");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) reset();
      }}
    >
      <DialogTrigger asChild>
        <Button size="sm">+ New connection</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New connection</DialogTitle>
          <DialogDescription>
            Direct TCP connection to a Postgres server. SSH options arrive in
            Task 7.
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-2 gap-3">
          <Field label="Name">
            <Input
              value={draft.name}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              placeholder="oci-prod"
            />
          </Field>
          <Field label="Database">
            <Input
              value={draft.database}
              onChange={(e) => setDraft({ ...draft, database: e.target.value })}
            />
          </Field>
          <Field label="Host">
            <Input
              value={draft.host}
              onChange={(e) => setDraft({ ...draft, host: e.target.value })}
            />
          </Field>
          <Field label="Port">
            <Input
              type="number"
              value={draft.port}
              onChange={(e) =>
                setDraft({ ...draft, port: Number(e.target.value) || 0 })
              }
            />
          </Field>
          <Field label="User">
            <Input
              value={draft.dbUser}
              onChange={(e) => setDraft({ ...draft, dbUser: e.target.value })}
            />
          </Field>
          <Field label="Password">
            <Input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          <Field label="SSL mode">
            <Select
              value={draft.sslMode}
              onValueChange={(v) => setDraft({ ...draft, sslMode: v })}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {[
                  "disable",
                  "allow",
                  "prefer",
                  "require",
                  "verify-ca",
                  "verify-full",
                ].map((m) => (
                  <SelectItem key={m} value={m}>
                    {m}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
        </div>

        <DialogFooter>
          <Button
            variant="ghost"
            onClick={() => setOpen(false)}
            disabled={busy}
          >
            Cancel
          </Button>
          <Button onClick={onSave} disabled={busy}>
            {busy ? "Saving..." : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1">
      <Label className="text-muted-foreground text-xs">{label}</Label>
      {children}
    </div>
  );
}
