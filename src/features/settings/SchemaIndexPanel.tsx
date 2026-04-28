import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { useAi } from "@/store/ai";
import { useConnections } from "@/store/connections";
import { useSchemaIndex } from "@/store/schemaIndex";
import { useSettings } from "@/store/settings";

export function SchemaIndexPanel() {
  const activeId = useConnections((s) => s.activeId);
  const connections = useConnections((s) => s.items);
  const progress = useSchemaIndex((s) =>
    activeId ? s.byConn[activeId] : undefined,
  );
  const ai = useAi((s) => s.providers);
  const defaultEmbed = useSettings((s) => s.defaultEmbeddingProvider);
  const auto = useSettings((s) => s.schemaIndexAutoSync);
  const setAuto = useSettings((s) => s.setSchemaIndexAutoSync);
  const [busy, setBusy] = useState(false);

  if (!activeId) {
    return (
      <p className="text-muted-foreground text-xs">
        No active connection. Open a connection first.
      </p>
    );
  }
  const connName = connections.find((c) => c.id === activeId)?.name ?? activeId;
  const provider = ai[defaultEmbed];

  const start = async () => {
    if (!provider.embeddingModel) {
      toast.error(`No embedding model configured for ${defaultEmbed}`);
      return;
    }
    setBusy(true);
    try {
      const r = await invoke<{
        embedded: number;
        skippedUnchanged: number;
        failed: string[];
      }>("sync_schema_index", {
        connectionId: activeId,
        embeddingProvider: defaultEmbed,
        embeddingModel: provider.embeddingModel,
        baseUrl: provider.baseUrl,
      });
      toast.success(
        `Schema index: ${r.embedded} embedded, ${r.skippedUnchanged} skipped, ${r.failed.length} failed`,
      );
    } catch (e) {
      toast.error(`Sync failed: ${e instanceof Error ? e.message : e}`);
    } finally {
      setBusy(false);
    }
  };

  const clear = async () => {
    try {
      await invoke("schema_index_clear", { connectionId: activeId });
      useSchemaIndex.getState().clear(activeId);
      toast.success("Schema index cleared");
    } catch (e) {
      toast.error(`Clear failed: ${e instanceof Error ? e.message : e}`);
    }
  };

  return (
    <div className="space-y-3 text-xs">
      <h2 className="font-medium">Connection: {connName}</h2>
      <p>
        Embedding provider: {defaultEmbed} · Model:{" "}
        {provider.embeddingModel ?? "—"}
      </p>
      {progress && (
        <p>
          State: {progress.state} · {progress.embeddedTables}/
          {progress.totalTables}
          {progress.errorMessage ? ` · err: ${progress.errorMessage}` : ""}
        </p>
      )}
      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={auto}
          onChange={(e) => setAuto(e.target.checked)}
        />
        Auto-sync on connect
      </label>
      <div className="flex gap-2">
        <Button size="sm" disabled={busy} onClick={start}>
          {busy ? "Syncing…" : "Rebuild now"}
        </Button>
        <Button size="sm" variant="ghost" onClick={clear}>
          Clear
        </Button>
      </div>
    </div>
  );
}
