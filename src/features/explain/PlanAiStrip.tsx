import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { buildModel } from "@/lib/ai/providers";
import { aiSecretGet } from "@/lib/keychain";
import {
  SYSTEM_EXPLAIN_PROMPT,
  buildExplainUserPrompt,
  type RelationContext,
} from "@/lib/ai/explainPrompts";
import { streamExplainInterpretation } from "@/lib/ai/explainStream";
import { planSha } from "@/lib/explain/planSha";
import type {
  AiInterpretation,
  ExplainResult,
  IndexCandidate,
} from "@/lib/explain/planTypes";
import { useAi } from "@/store/ai";
import { useSettings } from "@/store/settings";
import { useTabs } from "@/store/tabs";

interface Props {
  tabId: string;
  connId: string;
  result: ExplainResult;
  sql: string;
}

interface IndexRow {
  name: string;
  definition: string;
  is_unique: boolean;
  is_primary: boolean;
}

export function PlanAiStrip({ tabId, connId, result, sql }: Props) {
  const tab = useTabs((s) => s.tabs.find((t) => t.id === tabId));
  const cacheAi = useTabs((s) => s.cacheAi);
  const setActiveAiKey = useTabs((s) => s.setActiveAiKey);
  const ai = useAi((s) => s.providers);
  const provider = useSettings((s) => s.defaultGenerationProvider);
  const explainTokenBudget = useSettings((s) => s.explainTokenBudget);
  const autoInterpretPlan = useSettings((s) => s.autoInterpretPlan);
  const cfg = ai[provider];
  const model = cfg.generationModel;
  const [busy, setBusy] = useState(false);
  const [streamed, setStreamed] = useState("");
  const ctrlRef = useRef<AbortController | null>(null);
  const [cacheKey, setCacheKey] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void planSha({ plan: result.planJson, provider, model }).then((sha) => {
      if (!cancelled) setCacheKey(sha);
    });
    return () => {
      cancelled = true;
    };
  }, [result.planJson, provider, model]);

  const cached = useMemo(() => {
    if (!cacheKey) return null;
    return tab?.lastPlan?.aiCacheByKey[cacheKey] ?? null;
  }, [tab, cacheKey]);

  const doInterpret = async () => {
    if (!cacheKey) return;
    setBusy(true);
    setStreamed("");
    ctrlRef.current?.abort();
    const ctrl = new AbortController();
    ctrlRef.current = ctrl;

    try {
      const apiKey = await aiSecretGet(provider);
      if (!apiKey && provider !== "ollama") {
        toast.error(`${provider} key not set — open Settings`);
        return;
      }
      const relations = await fetchRelations(connId, result.verifiedCandidates);
      const userPrompt = buildExplainUserPrompt({
        result,
        sql,
        relations,
        tokenBudget: explainTokenBudget,
      });
      const m = buildModel({
        provider,
        modelId: model,
        apiKey: apiKey ?? "",
        baseUrl: cfg.baseUrl,
      });

      const interp: AiInterpretation = await streamExplainInterpretation({
        model: m,
        systemPrompt: SYSTEM_EXPLAIN_PROMPT,
        userPrompt,
        signal: ctrl.signal,
        onChunk: setStreamed,
      });

      cacheAi(tabId, cacheKey, interp);
      setActiveAiKey(tabId, cacheKey);

      await invoke("record_ai_explain", {
        payload: {
          connId,
          planSha: cacheKey,
          provider,
          model,
          summary: interp.summary,
          rawPlanJson: JSON.stringify(result.planJson),
          verifiedCandidatesJson: JSON.stringify(result.verifiedCandidates),
          llmRecommendationsJson: JSON.stringify(interp.recommendations),
          promptTokens: interp.promptTokens ?? null,
          completionTokens: interp.completionTokens ?? null,
          durationMs: interp.durationMs,
        },
      }).catch((e) =>
        toast.error(
          `Failed to record AI explain: ${e instanceof Error ? e.message : e}`,
        ),
      );
    } catch (e) {
      if (ctrl.signal.aborted) {
        toast("Interpretation cancelled");
      } else {
        toast.error(
          `Interpretation failed: ${e instanceof Error ? e.message : e}`,
        );
      }
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if (!cacheKey || cached || busy || !autoInterpretPlan) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect, react-hooks/exhaustive-deps
    void doInterpret();
  }, [cacheKey, cached, autoInterpretPlan]);

  return (
    <div className="border-border bg-muted/30 flex flex-col gap-2 border-t p-3 text-xs">
      <div className="flex items-center gap-2">
        <span className="font-semibold">AI interpretation</span>
        <span className="text-muted-foreground">
          {provider} · {model}
        </span>
        {!cached && (
          <Button
            size="sm"
            disabled={busy}
            onClick={doInterpret}
            className="ml-auto"
          >
            {busy ? "Streaming…" : "Interpret with AI"}
          </Button>
        )}
        {cached && (
          <Button
            size="sm"
            variant="ghost"
            className="ml-auto"
            onClick={doInterpret}
            disabled={busy}
            title="Re-run interpretation"
          >
            Re-run
          </Button>
        )}
      </div>
      <div className="leading-relaxed whitespace-pre-wrap">
        {cached?.summary ?? streamed ?? ""}
      </div>
      {cached && cached.recommendations.length > 0 && (
        <div>
          <div className="text-muted-foreground mb-1">AI recommendations</div>
          <ul className="ml-4 list-disc">
            {cached.recommendations.map((r, i) => (
              <li key={i}>
                <span className="font-mono">
                  {r.schema}.{r.table}({r.columns.join(", ")})
                </span>{" "}
                — {r.priority} · {r.reason}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

async function fetchRelations(
  connId: string,
  candidates: IndexCandidate[],
): Promise<RelationContext[]> {
  const set = new Map<string, RelationContext>();
  for (const c of candidates) {
    const key = `${c.schema}.${c.table}`;
    if (set.has(key)) continue;
    try {
      const ddl = await invoke<string>("get_table_schema", {
        connectionId: connId,
        schema: c.schema,
        table: c.table,
      });
      const indexes = await invoke<IndexRow[]>("list_indexes", {
        connectionId: connId,
        schema: c.schema,
        table: c.table,
      }).catch(() => [] as IndexRow[]);
      set.set(key, {
        schema: c.schema,
        table: c.table,
        ddl,
        indexes: indexes.map((i) => `${i.name}: ${i.definition}`),
        stats: {},
      });
    } catch {
      // best-effort — relation context is optional.
    }
  }
  return [...set.values()];
}
