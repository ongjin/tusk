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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useConnections } from "@/store/connections";
import type { NewConnection } from "@/lib/types";

import { SshHostPicker } from "./SshHostPicker";

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

  function commonFields() {
    return (
      <>
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
        <Field label="Postgres host">
          <Input
            value={draft.host}
            onChange={(e) => setDraft({ ...draft, host: e.target.value })}
          />
        </Field>
        <Field label="Postgres port">
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
      </>
    );
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
            Configure a Direct TCP or SSH-tunneled Postgres connection.
          </DialogDescription>
        </DialogHeader>

        <Tabs
          value={
            draft.sshKind === "None"
              ? "tcp"
              : draft.sshKind === "Alias"
                ? "alias"
                : "manual"
          }
          onValueChange={(v) => {
            if (v === "tcp") setDraft({ ...draft, sshKind: "None" });
            if (v === "alias") setDraft({ ...draft, sshKind: "Alias" });
            if (v === "manual") setDraft({ ...draft, sshKind: "Manual" });
          }}
        >
          <TabsList>
            <TabsTrigger value="tcp">Direct TCP</TabsTrigger>
            <TabsTrigger value="alias">SSH alias</TabsTrigger>
            <TabsTrigger value="manual">SSH manual</TabsTrigger>
          </TabsList>

          <TabsContent value="tcp" className="grid grid-cols-2 gap-3 pt-3">
            {commonFields()}
          </TabsContent>

          <TabsContent value="alias" className="flex flex-col gap-3 pt-3">
            <SshHostPicker
              selectedAlias={draft.sshAlias}
              onSelect={(host) =>
                setDraft({
                  ...draft,
                  sshAlias: host.alias,
                  sshHost: host.hostname,
                  sshUser: host.user,
                  sshPort: host.port,
                })
              }
            />
            <div className="text-muted-foreground text-xs">
              Selected: <strong>{draft.sshAlias ?? "—"}</strong>
            </div>
            <div className="grid grid-cols-2 gap-3">{commonFields()}</div>
          </TabsContent>

          <TabsContent value="manual" className="flex flex-col gap-3 pt-3">
            <div className="grid grid-cols-2 gap-3">
              <Field label="SSH host">
                <Input
                  value={draft.sshHost ?? ""}
                  onChange={(e) =>
                    setDraft({ ...draft, sshHost: e.target.value || null })
                  }
                />
              </Field>
              <Field label="SSH port">
                <Input
                  type="number"
                  value={draft.sshPort ?? 22}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      sshPort: Number(e.target.value) || 22,
                    })
                  }
                />
              </Field>
              <Field label="SSH user">
                <Input
                  value={draft.sshUser ?? ""}
                  onChange={(e) =>
                    setDraft({ ...draft, sshUser: e.target.value || null })
                  }
                />
              </Field>
              <Field label="SSH key path">
                <Input
                  value={draft.sshKeyPath ?? ""}
                  placeholder="~/.ssh/id_ed25519"
                  onChange={(e) =>
                    setDraft({ ...draft, sshKeyPath: e.target.value || null })
                  }
                />
              </Field>
            </div>
            <div className="grid grid-cols-2 gap-3">{commonFields()}</div>
          </TabsContent>
        </Tabs>

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
