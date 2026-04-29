import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { extractSql, buildSystemPrompt } from "@/lib/ai/prompts";
import { buildModel } from "@/lib/ai/providers";
import { streamGeneration } from "@/lib/ai/stream";
import { aiSecretGet } from "@/lib/keychain";
import type { SchemaTopK } from "@/lib/ai/types";
import { useAi } from "@/store/ai";
import { useSettings } from "@/store/settings";
import { SqlDiffView } from "./SqlDiffView";

export interface ApplyMeta {
  prompt: string;
  generatedSql: string;
  topKTables: string[];
  toolCalls: { name: string; args: unknown }[];
  provider: string;
  generationModel: string;
  embeddingModel?: string;
  promptTokens?: number;
  completionTokens?: number;
}

interface Props {
  open: boolean;
  connectionId: string | undefined;
  selection: string;
  onClose: () => void;
  onApply: (sql: string, meta: ApplyMeta) => void;
}

export function CmdKPalette({
  open,
  connectionId,
  selection,
  onClose,
  onApply,
}: Props) {
  const [prompt, setPrompt] = useState("");
  const [streamed, setStreamed] = useState("");
  const [busy, setBusy] = useState(false);
  const [meta, setMeta] = useState<ApplyMeta | null>(null);
  const ctrlRef = useRef<AbortController | null>(null);
  const ai = useAi((s) => s.providers);
  const setLastPrompt = useAi((s) => s.setLastPrompt);
  const lastPrompt = useAi((s) => s.lastPrompt);
  const settings = useSettings();
  const defaultGen = settings.defaultGenerationProvider;
  const defaultEmbed = settings.defaultEmbeddingProvider;
  const sampleRowsEnabled = settings.toolsEnabled.sampleRows;
  const ragTopK = settings.ragTopK;

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    if (open) setPrompt(lastPrompt);
  }, [open, lastPrompt]);

  useEffect(() => {
    return () => ctrlRef.current?.abort();
  }, []);

  if (!open) return null;

  const cfg = ai[defaultGen];
  const embedCfg = ai[defaultEmbed];
  const noKey = !cfg.apiKeyPresent && defaultGen !== "ollama";

  const onSubmit = async () => {
    if (!connectionId) {
      toast.error("No active connection");
      return;
    }
    if (noKey) {
      toast.error(`${defaultGen} key not set — open Settings`);
      return;
    }
    if (!embedCfg.embeddingModel) {
      toast.error(`Embedding model not set for ${defaultEmbed}`);
      return;
    }
    setBusy(true);
    setStreamed("");
    setMeta(null);
    setLastPrompt(prompt);
    ctrlRef.current?.abort();
    const ctrl = new AbortController();
    ctrlRef.current = ctrl;

    try {
      let apiKey = await aiSecretGet(defaultGen);
      const topK = await invoke<SchemaTopK>("schema_top_k", {
        connectionId,
        userPrompt: prompt,
        embeddingProvider: defaultEmbed,
        embeddingModel: embedCfg.embeddingModel,
        baseUrl: embedCfg.baseUrl,
        topK: ragTopK,
      });
      const recent = await invoke<{ sqlPreview: string }[]>(
        "list_recent_successful",
        { connectionId, limit: 5 },
      );
      const pgVersion = "16";
      const extensions: string[] = [];
      const systemPrompt = buildSystemPrompt({
        pgVersion,
        extensions,
        topK: topK.tables,
        recentSuccessful: recent.map((r) => r.sqlPreview),
        selectionContext: selection || undefined,
      });
      const model = buildModel({
        provider: defaultGen,
        modelId: cfg.generationModel,
        apiKey: apiKey ?? "",
        baseUrl: cfg.baseUrl,
      });
      apiKey = null;

      const r = await streamGeneration({
        model,
        systemPrompt,
        userPrompt: prompt,
        connectionId,
        sampleRowsEnabled,
        signal: ctrl.signal,
        onChunk: (txt) => setStreamed(txt),
      });
      const sql = extractSql(r.text);
      setStreamed(sql);
      setMeta({
        prompt,
        generatedSql: sql,
        topKTables: topK.tables.map((t) => `${t.schema}.${t.table}`),
        toolCalls: r.toolCalls,
        provider: defaultGen,
        generationModel: cfg.generationModel,
        embeddingModel: embedCfg.embeddingModel,
        promptTokens: r.promptTokens,
        completionTokens: r.completionTokens,
      });
    } catch (e) {
      if (ctrl.signal.aborted) {
        toast("Generation cancelled");
      } else {
        toast.error(`Generation failed: ${e instanceof Error ? e.message : e}`);
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-40" onClick={onClose} role="presentation">
      <div
        className="bg-card fixed top-1/4 left-1/2 z-50 w-[640px] -translate-x-1/2 rounded border p-3 shadow"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="Cmd+K"
      >
        <div className="flex items-center gap-2">
          <span aria-hidden>✦</span>
          <input
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !busy) {
                e.preventDefault();
                void onSubmit();
              } else if (e.key === "Escape") {
                onClose();
              }
            }}
            placeholder={
              selection
                ? "Edit the selected SQL…"
                : "Generate SQL from natural language…"
            }
            className="border-input flex-1 rounded border px-2 py-1 text-sm"
            autoFocus
          />
          <Button
            size="sm"
            disabled={busy || prompt.trim().length === 0}
            onClick={onSubmit}
          >
            {busy ? "Streaming…" : "Generate"}
          </Button>
        </div>
        <p className="text-muted-foreground mt-1 text-xs">
          {defaultGen} · {cfg.generationModel} · top-K {ragTopK}
        </p>
        {streamed &&
          (selection ? (
            <div className="mt-3">
              <SqlDiffView original={selection} modified={streamed} />
            </div>
          ) : (
            <pre className="bg-muted mt-3 max-h-64 overflow-auto rounded p-2 text-xs">
              {streamed}
            </pre>
          ))}
        {meta && (
          <div className="mt-3 flex justify-end gap-2">
            <Button
              variant="ghost"
              onClick={() => {
                ctrlRef.current?.abort();
                setStreamed("");
                setMeta(null);
              }}
            >
              Re-prompt
            </Button>
            <Button variant="ghost" onClick={onClose}>
              Reject
            </Button>
            <Button
              onClick={() => {
                if (meta) onApply(meta.generatedSql, meta);
              }}
            >
              Apply
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
