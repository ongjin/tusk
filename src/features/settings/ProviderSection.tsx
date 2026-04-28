import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { aiSecretSet, aiSecretDelete } from "@/lib/keychain";
import type { AiProvider, ProviderConfig } from "@/lib/types";
import { useAi } from "@/store/ai";
import { useSettings } from "@/store/settings";

const PROVIDERS: { id: AiProvider; label: string; needsBaseUrl: boolean }[] = [
  { id: "openai", label: "OpenAI", needsBaseUrl: false },
  { id: "anthropic", label: "Anthropic", needsBaseUrl: false },
  { id: "gemini", label: "Gemini", needsBaseUrl: false },
  { id: "ollama", label: "Ollama", needsBaseUrl: true },
];

export function ProviderSection() {
  const providers = useAi((s) => s.providers);
  const setProviderConfig = useAi((s) => s.setProviderConfig);
  const enabledProviders = useSettings((s) => s.enabledProviders);
  const setEnabledProviders = useSettings((s) => s.setEnabledProviders);
  const defaultGen = useSettings((s) => s.defaultGenerationProvider);
  const defaultEmbed = useSettings((s) => s.defaultEmbeddingProvider);
  const setDefaultGen = useSettings((s) => s.setDefaultGenerationProvider);
  const setDefaultEmbed = useSettings((s) => s.setDefaultEmbeddingProvider);
  const sampleRowsEnabled = useSettings((s) => s.toolsEnabled.sampleRows);
  const setSampleRows = useSettings((s) => s.setSampleRowsEnabled);
  const destructiveStrict = useSettings((s) => s.destructiveStrict);
  const setDestructiveStrict = useSettings((s) => s.setDestructiveStrict);

  return (
    <div className="space-y-4 text-xs">
      {PROVIDERS.map((meta) => (
        <ProviderCard
          key={meta.id}
          providerId={meta.id}
          label={meta.label}
          needsBaseUrl={meta.needsBaseUrl}
          config={providers[meta.id]}
          enabled={enabledProviders.includes(meta.id)}
          onToggle={(v) => {
            if (v) {
              if (!enabledProviders.includes(meta.id)) {
                setEnabledProviders([...enabledProviders, meta.id]);
              }
            } else {
              setEnabledProviders(
                enabledProviders.filter((p) => p !== meta.id),
              );
            }
          }}
          onSave={async (key) => {
            try {
              await aiSecretSet(meta.id, key);
              setProviderConfig(meta.id, { apiKeyPresent: true });
              if (!enabledProviders.includes(meta.id)) {
                setEnabledProviders([...enabledProviders, meta.id]);
              }
              toast.success(`${meta.label} key saved`);
            } catch (e) {
              toast.error(`Failed to save: ${asMessage(e)}`);
            }
          }}
          onDelete={async () => {
            try {
              await aiSecretDelete(meta.id);
              setProviderConfig(meta.id, { apiKeyPresent: false });
              setEnabledProviders(
                enabledProviders.filter((p) => p !== meta.id),
              );
              toast.success(`${meta.label} key removed`);
            } catch (e) {
              toast.error(`Failed to delete: ${asMessage(e)}`);
            }
          }}
          onConfigChange={(patch) => setProviderConfig(meta.id, patch)}
          onTest={() => {
            toast("Test stub — wired in Task 6");
          }}
        />
      ))}

      <div className="border-border flex flex-col gap-2 border-t pt-4">
        <label className="flex items-center justify-between">
          <span>Default generation provider</span>
          <select
            className="border-input rounded border px-2 py-1"
            value={defaultGen}
            onChange={(e) => setDefaultGen(e.target.value as AiProvider)}
          >
            {PROVIDERS.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center justify-between">
          <span>Default embedding provider</span>
          <select
            className="border-input rounded border px-2 py-1"
            value={defaultEmbed}
            onChange={(e) => setDefaultEmbed(e.target.value as AiProvider)}
          >
            {PROVIDERS.filter((p) => p.id !== "anthropic").map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
              </option>
            ))}
          </select>
        </label>
        {defaultEmbed === "anthropic" && (
          <p className="text-amber-600">
            Anthropic does not provide embeddings. Pick another provider.
          </p>
        )}
      </div>

      <div className="border-border flex flex-col gap-2 border-t pt-4">
        <h3 className="font-medium">Tools</h3>
        <label className="flex items-center justify-between">
          <span>get_table_schema (always on)</span>
          <input type="checkbox" checked disabled />
        </label>
        <label className="flex items-center justify-between">
          <span>list_indexes (always on)</span>
          <input type="checkbox" checked disabled />
        </label>
        <label className="flex items-center justify-between">
          <span>
            sample_rows{" "}
            <span className="text-muted-foreground">(sends rows to LLM)</span>
          </span>
          <input
            type="checkbox"
            checked={sampleRowsEnabled}
            onChange={(e) => setSampleRows(e.target.checked)}
          />
        </label>
      </div>

      <div className="border-border flex flex-col gap-2 border-t pt-4">
        <h3 className="font-medium">Destructive query confirmation</h3>
        <label className="flex items-center gap-2">
          <input
            type="radio"
            checked={!destructiveStrict}
            onChange={() => setDestructiveStrict(false)}
          />
          <span>Standard — Cancel / Run anyway</span>
        </label>
        <label className="flex items-center gap-2">
          <input
            type="radio"
            checked={destructiveStrict}
            onChange={() => setDestructiveStrict(true)}
          />
          <span>Strict — type the keyword to confirm</span>
        </label>
      </div>
    </div>
  );
}

function asMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

interface CardProps {
  providerId: AiProvider;
  label: string;
  needsBaseUrl: boolean;
  config: ProviderConfig;
  enabled: boolean;
  onToggle: (v: boolean) => void;
  onSave: (key: string) => Promise<void>;
  onDelete: () => Promise<void>;
  onConfigChange: (patch: Partial<ProviderConfig>) => void;
  onTest: () => void;
}

function ProviderCard(p: CardProps) {
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  return (
    <div className="border-border rounded border p-3">
      <div className="flex items-center justify-between">
        <label className="flex items-center gap-2 font-medium">
          <input
            type="checkbox"
            checked={p.enabled}
            onChange={(e) => p.onToggle(e.target.checked)}
          />
          {p.label}
          {p.config.apiKeyPresent ? (
            <span className="text-emerald-600">· key set</span>
          ) : (
            <span className="text-muted-foreground">· no key</span>
          )}
        </label>
        {p.config.apiKeyPresent && (
          <Button size="sm" variant="ghost" onClick={p.onTest}>
            Test
          </Button>
        )}
      </div>
      {p.providerId !== "ollama" && (
        <div className="mt-2 flex items-center gap-2">
          <input
            type="password"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder={
              p.config.apiKeyPresent ? "(stored — paste to replace)" : "API key"
            }
            className="border-input flex-1 rounded border px-2 py-1"
          />
          <Button
            size="sm"
            disabled={busy || key.length === 0}
            onClick={async () => {
              setBusy(true);
              await p.onSave(key);
              setKey("");
              setBusy(false);
            }}
          >
            Save
          </Button>
          {p.config.apiKeyPresent && (
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                await p.onDelete();
                setBusy(false);
              }}
            >
              Remove
            </Button>
          )}
        </div>
      )}
      {p.needsBaseUrl && (
        <label className="mt-2 flex items-center gap-2">
          <span className="w-16">Base URL</span>
          <input
            value={p.config.baseUrl ?? ""}
            onChange={(e) => p.onConfigChange({ baseUrl: e.target.value })}
            className="border-input flex-1 rounded border px-2 py-1"
          />
        </label>
      )}
      <label className="mt-2 flex items-center gap-2">
        <span className="w-32">Generation model</span>
        <input
          value={p.config.generationModel}
          onChange={(e) =>
            p.onConfigChange({ generationModel: e.target.value })
          }
          className="border-input flex-1 rounded border px-2 py-1"
        />
      </label>
      {p.providerId !== "anthropic" && (
        <label className="mt-2 flex items-center gap-2">
          <span className="w-32">Embedding model</span>
          <input
            value={p.config.embeddingModel ?? ""}
            onChange={(e) =>
              p.onConfigChange({ embeddingModel: e.target.value || undefined })
            }
            className="border-input flex-1 rounded border px-2 py-1"
          />
        </label>
      )}
      {p.providerId === "anthropic" && (
        <p className="text-muted-foreground mt-2">
          Anthropic has no native embeddings — pick another embedding provider
          below.
        </p>
      )}
    </div>
  );
}
