# Week 4 — AI 1차: BYOK + 자연어→SQL + 스키마 RAG + Destructive Guard — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tusk를 AI 1급 시민으로 진화시킨다. OS keychain에 4 provider(OpenAI/Anthropic/Gemini/Ollama) BYOK → 연결 시 스키마 임베딩 자동 빌드 → 에디터 `Cmd+K` 자연어 입력 → 관련 테이블 top-K DDL을 system prompt에 첨부해 streaming SQL 생성 → diff view → Apply. AI가 만든 SQL뿐 아니라 사용자가 직접 친 SQL도 동일한 AST 기반 destructive 게이트(DROP/TRUNCATE/DELETE-no-where 등)를 통과해야 실행된다.

**Architecture:** Generation은 frontend (Vercel AI SDK 6 `streamText` + tool calling), embedding은 Rust(`reqwest` REST → rusqlite BLOB `f32[]` little-endian + 인메모리 cosine). 키는 매 호출마다 keychain fetch + 변수 즉시 drop, frontend엔 `apiKeyPresent: bool`만 영속. Destructive 검출은 `sqlparser` AST 순회로 multi-statement 안에서도 단계별 finding을 모음. 임베딩 인덱스는 `(conn_id, schema, table, ddl_checksum, embedding_model)` 매칭 시 SKIP하고 그 외엔 incremental re-embed.

**Tech Stack:**
- Frontend new deps: `@ai-sdk/google` (Gemini), `ollama-ai-provider-v2` (Ollama AI SDK adapter), `zod` (이미 transitive로 들어와 있을 가능성 — 명시적 추가).
- Rust new deps: `reqwest 0.12` (rustls features), `bytemuck 1` (f32 ↔ &[u8] cast 안전).
- Existing: `sqlparser 0.52` (Week 3에서 이미 추가, destructive AST 재사용), `rusqlite 0.32`, `keyring 3`, `sqlx 0.8`.

**Reference spec:** `docs/superpowers/specs/2026-04-28-week-4-ai-natural-language-sql-design.md`.

**Working dir:** `/Users/cyj/workspace/personal/tusk` on `main`.

**Branching:** 사용자 지시에 따라 main에서 직접 작업. Week 3 implementation이 main에 아직 없으므로 **Phase 0의 prereq 머지가 선행되어야 함** (T0 참조).

**Quality gates between tasks:**

```
pnpm typecheck && pnpm lint && pnpm format:check
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
```

Run only the gates relevant to the task (Rust task → rust:\* + cargo test; Frontend task → typecheck/lint/format + `pnpm build`). Final task runs the full set.

**Integration tests with docker postgres:** Same `infra/postgres/docker-compose.yml`. Connection: `postgres://tusk:tusk@127.0.0.1:55432/tusk_test`.

**Mock embedding endpoint:** Embedding network calls in Rust integration tests are stubbed with `httpmock 0.7`.

**Commit message convention:** Conventional commits (`feat`, `fix`, `chore`, `docs`, `refactor`, `test`). **Do NOT add `Co-Authored-By` trailers or "Generated with ..." lines.** Commit messages describe the change, nothing else.

---

## File structure (created during this plan)

```
src-tauri/src/
  commands/
    ai_secrets.rs         (T2)  — provider별 keychain set/get/delete/list
    destructive.rs        (T7)  — classify_destructive (AST)
    schema_index.rs       (T15) — sync_schema_index, schema_top_k, list_recent_queries
    ai_tools.rs           (T19) — get_table_schema / list_indexes / sample_rows
    history.rs            (모디파이 — T18) — ai_history insert
    query.rs              (모디파이 — T10) — destructive 게이트 진입
    mod.rs                (모디파이 매번)
  db/
    schema_embed.rs       (T12) — build_table_ddl + checksum
    embedding_store.rs    (T13) — BLOB read/write + cosine top_k
    state.rs              (모디파이 — T11) — migration 003_ai
  secrets.rs              (모디파이 — T1) — ai_entry helper
  errors.rs               (모디파이 — T1) — Ai/SchemaIndex/EmbeddingHttp/Destructive variants
  lib.rs                  (모디파이 매 command-add)
  Cargo.toml              (모디파이 — T1)
  tests/
    destructive.rs        (T7)
    schema_index.rs       (T15)
    ai_tools.rs           (T19)
    ai_secrets.rs         (T2)

src/
  lib/
    ai/
      providers.ts        (T5)  — AI SDK model factory (4 providers)
      prompts.ts          (T18) — system prompt assembly
      tools.ts            (T18) — AI SDK tool defs
      destructive.ts      (T8)  — fast regex pre-warn (mirror, not gate)
      stream.ts           (T20) — streamText wrapper + AbortController
    keychain.ts           (T2)  — invoke wrapper for ai_secrets
    types.ts              (모디파이 — T1) — AiProvider, ProviderConfig, etc.
  store/
    ai.ts                 (T3)  — provider config + last prompt + abort controller
    schemaIndex.ts        (T16) — progress mirror
    settings.ts           (모디파이 — T3) — enabledProviders, defaults, ragTopK, destructiveStrict
  features/
    settings/
      SettingsDialog.tsx  (T4)
      ProviderSection.tsx (T4)
      ModelPicker.tsx     (T5)
      SchemaIndexPanel.tsx (T16)
    ai/
      DestructiveModal.tsx (T9)
      CmdKPalette.tsx     (T20)
      SqlDiffView.tsx     (T21)
      AiHistoryEntry.tsx  (T22)
    editor/
      EditorPane.tsx      (모디파이 — T22) — Cmd+K 단축키
    history/
      HistoryPalette.tsx  (모디파이 — T22) — 'ai' source 렌더

docs/superpowers/plans/
  2026-04-28-week-4-ai-natural-language-sql.md
  manual-verification-week-4.md  (T24)
```

---

## Task 0: Prerequisites — Week 3 implementation merged to main

**Goal:** Week 4 spec assumes Week 3 features exist on main: history tables (`migration 002_history`), `sqlparser` dep, `commands/sqlast.rs`, `db/decoder.rs`, `commands/transactions.rs`, `secrets.rs` ai_entry-ready helper. Without these, Week 4 tasks reference symbols that don't exist.

**Files:** none (operational task)

**Steps:**

- [ ] **Step 1: Verify Week 3 branch state**

```bash
git log --oneline feat/week3-result-editing | head -10
git diff main..feat/week3-result-editing --stat
```

Expected: Week 3 implementation diff present (decoder.rs / editing.rs / transactions.rs / pg_literals.rs / pg_meta.rs / sqlast.rs / migration 002_history / history.rs / cancel.rs / export.rs / settings store / pendingChanges store / vitest setup). Week 3 manual verification doc passes.

- [ ] **Step 2: Merge Week 3 → main**

If Week 3 verification + review已 통과:

```bash
git checkout main
git merge --no-ff feat/week3-result-editing -m "feat: week 3 — result inline editing + explicit transactions"
```

If conflicts: resolve referencing Week 3 plan's intent. None expected if main only has Week 3 spec/plan docs.

- [ ] **Step 3: Verify post-merge gate**

```bash
pnpm install
pnpm typecheck && pnpm lint && pnpm format:check
pnpm rust:check && pnpm rust:lint && pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored
```

All green expected.

- [ ] **Step 4: Confirm Week 4 spec landed**

```bash
ls docs/superpowers/specs/2026-04-28-week-4-ai-natural-language-sql-design.md
```

Should already exist (cherry-picked earlier). If not, cherry-pick from feat/week3-result-editing branch:

```bash
git cherry-pick 9202194  # adjust if hash drifted
```

---

## Task 1: Foundation — Cargo deps, error variants, secrets ai_entry, frontend types

**Goal:** All cross-cutting deps and types in place before any feature code.

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/errors.rs`
- Modify: `src-tauri/src/secrets.rs`
- Modify: `src/lib/types.ts`
- Modify: `package.json`

**Steps:**

- [ ] **Step 1: Add Rust deps**

Edit `src-tauri/Cargo.toml`, append to `[dependencies]`:

```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
bytemuck = { version = "1", features = ["derive"] }
httpmock = { version = "0.7", optional = true }
```

Add a new dev-dependency for tests:

```toml
[dev-dependencies]
# (existing entries kept)
httpmock = "0.7"
```

(`bytemuck` for safe `&[f32]` ↔ `&[u8]` casts in BLOB serialization. `reqwest` for embedding HTTP. `httpmock` for integration tests.)

- [ ] **Step 2: Verify Rust deps compile**

```bash
pnpm rust:check
```

Expected: success (just dep resolution, no code yet).

- [ ] **Step 3: Add error variants in `src-tauri/src/errors.rs`**

Append to the `TuskError` enum:

```rust
    #[error("AI provider error: {0}")]
    Ai(String),

    #[error("AI provider not configured: {0}")]
    AiNotConfigured(String),

    #[error("Schema index error: {0}")]
    SchemaIndex(String),

    #[error("Embedding HTTP error: {0}")]
    EmbeddingHttp(String),

    #[error("Destructive guard: parser failed")]
    DestructiveParserFailed,

    #[error("Destructive guard: confirmation required")]
    DestructiveConfirmRequired,
```

- [ ] **Step 4: Add `ai_entry` helper in `src-tauri/src/secrets.rs`**

Below the existing `entry(connection_id)` helper add:

```rust
fn ai_entry(provider: &str) -> TuskResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("ai:{provider}"))
        .map_err(|e| TuskError::Secrets(e.to_string()))
}

pub fn ai_set(provider: &str, value: &str) -> TuskResult<()> {
    ai_entry(provider)?
        .set_password(value)
        .map_err(|e| TuskError::Secrets(e.to_string()))
}

pub fn ai_get(provider: &str) -> TuskResult<Option<String>> {
    match ai_entry(provider)?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(TuskError::Secrets(e.to_string())),
    }
}

pub fn ai_delete(provider: &str) -> TuskResult<()> {
    match ai_entry(provider)?.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(TuskError::Secrets(e.to_string())),
    }
}
```

Validate provider name: only `["openai","anthropic","gemini","ollama"]` allowed. Add at top of each `ai_*` fn:

```rust
const ALLOWED_AI_PROVIDERS: &[&str] = &["openai", "anthropic", "gemini", "ollama"];
fn validate_provider(p: &str) -> TuskResult<()> {
    if ALLOWED_AI_PROVIDERS.contains(&p) {
        Ok(())
    } else {
        Err(TuskError::Ai(format!("unknown provider: {p}")))
    }
}
```

Call `validate_provider(provider)?` first inside each `ai_set`/`ai_get`/`ai_delete`.

- [ ] **Step 5: Write unit test for ai_entry roundtrip**

Append to `src-tauri/src/secrets.rs` `tests` mod:

```rust
    #[test]
    fn ai_set_get_delete_roundtrip() {
        let provider = "openai";
        // Best-effort cleanup before
        let _ = ai_delete(provider);
        if ai_set(provider, "sk-test-xyz").is_err() {
            eprintln!("skipping ai_set_get_delete_roundtrip: keyring backend unavailable");
            return;
        }
        let got = ai_get(provider).unwrap();
        assert_eq!(got.as_deref(), Some("sk-test-xyz"));
        ai_delete(provider).unwrap();
        assert_eq!(ai_get(provider).unwrap(), None);
    }

    #[test]
    fn ai_unknown_provider_rejected() {
        assert!(ai_set("oxygen", "x").is_err());
        assert!(ai_get("oxygen").is_err());
        assert!(ai_delete("oxygen").is_err());
    }
```

- [ ] **Step 6: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml secrets::tests
```

Expected: PASS (the keychain test may print "skipping" on Linux without a backend — acceptable).

- [ ] **Step 7: Add frontend AI types in `src/lib/types.ts`**

Append:

```ts
export type AiProvider = "openai" | "anthropic" | "gemini" | "ollama";

export interface ProviderConfig {
  provider: AiProvider;
  apiKeyPresent: boolean;
  baseUrl?: string;
  generationModel: string;
  embeddingModel?: string;
}

export type DestructiveKind =
  | "drop-database"
  | "drop-schema"
  | "drop-table"
  | "drop-column"
  | "drop-index"
  | "drop-view"
  | "drop-function"
  | "truncate"
  | "delete-no-where"
  | "update-no-where"
  | "alter-drop-constraint"
  | "grant-revoke-all"
  | "vacuum-full"
  | "parser-failed";

export interface DestructiveFinding {
  kind: DestructiveKind;
  statementIndex: number;
  message: string;
  affectedObject?: string;
}

export interface SchemaIndexProgress {
  connId: string;
  state: "idle" | "running" | "done" | "error";
  totalTables: number;
  embeddedTables: number;
  errorMessage?: string;
  lastSyncedAt?: number;
}

export interface AiHistoryMeta {
  source: "ai";
  provider: AiProvider;
  generationModel: string;
  embeddingModel?: string;
  prompt: string;
  generatedSql: string;
  topKTables: string[];
  toolCalls: { name: string; args: unknown }[];
  promptTokens?: number;
  completionTokens?: number;
}
```

- [ ] **Step 8: Add frontend new deps**

```bash
pnpm add @ai-sdk/google ollama-ai-provider-v2 zod
```

(Pin to versions compatible with `ai@^6`. If install fails, check the AI SDK 6 compat matrix in their docs; downgrade Gemini/Ollama provider to whatever ships an "ai" peer of `^6`.)

- [ ] **Step 9: Verify frontend compiles**

```bash
pnpm typecheck && pnpm lint
```

Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/errors.rs src-tauri/src/secrets.rs src/lib/types.ts package.json pnpm-lock.yaml
git commit -m "feat(week4): foundation — ai_entry keychain, error variants, ai types"
```

---

## Task 2: ai_secrets command surface + frontend keychain wrapper

**Goal:** `invoke('ai_secret_set'|'ai_secret_get'|'ai_secret_delete'|'ai_secret_list_present')` callable from frontend. Frontend never persists the raw key — only `apiKeyPresent: bool`.

**Files:**
- Create: `src-tauri/src/commands/ai_secrets.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/lib/keychain.ts`

**Steps:**

- [ ] **Step 1: Create the Rust command module**

Create `src-tauri/src/commands/ai_secrets.rs`:

```rust
//! Frontend-facing wrappers around `secrets::ai_*`.
//!
//! `ai_secret_get` is the only path that returns the raw key value; it must
//! be called only at the moment of LLM invocation and the value MUST NOT be
//! cached anywhere on the frontend.

use crate::errors::TuskResult;
use crate::secrets;

#[tauri::command]
pub fn ai_secret_set(provider: String, value: String) -> TuskResult<()> {
    secrets::ai_set(&provider, &value)
}

#[tauri::command]
pub fn ai_secret_get(provider: String) -> TuskResult<Option<String>> {
    secrets::ai_get(&provider)
}

#[tauri::command]
pub fn ai_secret_delete(provider: String) -> TuskResult<()> {
    secrets::ai_delete(&provider)
}

/// Returns the providers that currently have a key in the keychain. Used
/// by the Settings UI to render `apiKeyPresent: bool`.
#[tauri::command]
pub fn ai_secret_list_present() -> TuskResult<Vec<String>> {
    let providers = ["openai", "anthropic", "gemini", "ollama"];
    let mut present = Vec::new();
    for p in providers {
        match secrets::ai_get(p)? {
            Some(_) => present.push(p.to_string()),
            None => {}
        }
    }
    Ok(present)
}
```

- [ ] **Step 2: Register module**

Edit `src-tauri/src/commands/mod.rs`, add `pub mod ai_secrets;` (alphabetical order).

- [ ] **Step 3: Register tauri commands**

Edit `src-tauri/src/lib.rs` `invoke_handler!` macro, add:

```rust
            commands::ai_secrets::ai_secret_set,
            commands::ai_secrets::ai_secret_get,
            commands::ai_secrets::ai_secret_delete,
            commands::ai_secrets::ai_secret_list_present,
```

- [ ] **Step 4: Verify Rust builds**

```bash
pnpm rust:check && pnpm rust:lint
```

Expected: PASS.

- [ ] **Step 5: Write Rust integration test**

Create `src-tauri/tests/ai_secrets.rs`:

```rust
//! Smoke test for the keychain bridge. Skipped on hosts without a keyring.

use tusk_lib::secrets;

#[test]
fn roundtrip_each_provider() {
    for p in ["openai", "anthropic", "gemini", "ollama"] {
        let _ = secrets::ai_delete(p);
        if secrets::ai_set(p, "sk-test").is_err() {
            eprintln!("skipping {p}: keyring backend unavailable");
            continue;
        }
        assert_eq!(secrets::ai_get(p).unwrap().as_deref(), Some("sk-test"));
        secrets::ai_delete(p).unwrap();
        assert!(secrets::ai_get(p).unwrap().is_none());
    }
}

#[test]
fn unknown_provider_rejected() {
    assert!(secrets::ai_set("hydrogen", "x").is_err());
}
```

- [ ] **Step 6: Run integration test**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test ai_secrets
```

Expected: PASS (or print skip lines on Linux).

- [ ] **Step 7: Frontend keychain wrapper**

Create `src/lib/keychain.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

import type { AiProvider } from "@/lib/types";

export function aiSecretSet(provider: AiProvider, value: string) {
  return invoke<void>("ai_secret_set", { provider, value });
}

/** Returns the raw key. Caller MUST NOT cache. */
export function aiSecretGet(provider: AiProvider) {
  return invoke<string | null>("ai_secret_get", { provider });
}

export function aiSecretDelete(provider: AiProvider) {
  return invoke<void>("ai_secret_delete", { provider });
}

export function aiSecretListPresent() {
  return invoke<AiProvider[]>("ai_secret_list_present");
}
```

- [ ] **Step 8: Verify frontend compiles**

```bash
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/commands/ai_secrets.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/tests/ai_secrets.rs src/lib/keychain.ts
git commit -m "feat(week4): ai_secret_* tauri commands + frontend keychain wrapper"
```

---

## Task 3: ai store + extended settings store

**Goal:** Frontend state for provider configs, defaults, RAG topK, destructive mode, tools toggle. Persists everything **except** raw keys.

**Files:**
- Create: `src/store/ai.ts`
- Modify: `src/store/settings.ts`

**Steps:**

- [ ] **Step 1: Extend `settings.ts`**

Replace contents of `src/store/settings.ts` with:

```ts
import { create } from "zustand";
import { persist } from "zustand/middleware";

import type { AiProvider } from "@/lib/types";

interface SettingsState {
  // Existing — keep as-is
  autoLimit: number;
  setAutoLimit: (v: number) => void;
  editConflictMode: "pkOnly" | "strict";
  setEditConflictMode: (m: "pkOnly" | "strict") => void;

  // Week 4
  enabledProviders: AiProvider[];
  defaultGenerationProvider: AiProvider;
  defaultEmbeddingProvider: AiProvider;
  toolsEnabled: { sampleRows: boolean };
  destructiveStrict: boolean;
  ragTopK: number;
  schemaIndexAutoSync: boolean;
  setEnabledProviders: (v: AiProvider[]) => void;
  setDefaultGenerationProvider: (p: AiProvider) => void;
  setDefaultEmbeddingProvider: (p: AiProvider) => void;
  setSampleRowsEnabled: (v: boolean) => void;
  setDestructiveStrict: (v: boolean) => void;
  setRagTopK: (v: number) => void;
  setSchemaIndexAutoSync: (v: boolean) => void;
}

export const useSettings = create<SettingsState>()(
  persist(
    (set) => ({
      autoLimit: 1000,
      setAutoLimit: (v) => set({ autoLimit: v }),
      editConflictMode: "pkOnly",
      setEditConflictMode: (m) => set({ editConflictMode: m }),

      enabledProviders: [],
      defaultGenerationProvider: "openai",
      defaultEmbeddingProvider: "openai",
      toolsEnabled: { sampleRows: false },
      destructiveStrict: false,
      ragTopK: 8,
      schemaIndexAutoSync: true,
      setEnabledProviders: (v) => set({ enabledProviders: v }),
      setDefaultGenerationProvider: (p) =>
        set({ defaultGenerationProvider: p }),
      setDefaultEmbeddingProvider: (p) => set({ defaultEmbeddingProvider: p }),
      setSampleRowsEnabled: (v) =>
        set((s) => ({ toolsEnabled: { ...s.toolsEnabled, sampleRows: v } })),
      setDestructiveStrict: (v) => set({ destructiveStrict: v }),
      setRagTopK: (v) => set({ ragTopK: Math.max(1, Math.min(32, v)) }),
      setSchemaIndexAutoSync: (v) => set({ schemaIndexAutoSync: v }),
    }),
    { name: "tusk-settings" },
  ),
);
```

- [ ] **Step 2: Create `src/store/ai.ts`**

```ts
import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";

import type { AiProvider, ProviderConfig } from "@/lib/types";

interface AiState {
  /** Per-provider config. Raw API key is NEVER stored — only apiKeyPresent. */
  providers: Record<AiProvider, ProviderConfig>;
  setProviderConfig: (p: AiProvider, patch: Partial<ProviderConfig>) => void;
  /** Most recent NL prompt (for re-prompt UI). */
  lastPrompt: string;
  setLastPrompt: (s: string) => void;
}

const defaults: Record<AiProvider, ProviderConfig> = {
  openai: {
    provider: "openai",
    apiKeyPresent: false,
    generationModel: "gpt-4o-mini",
    embeddingModel: "text-embedding-3-small",
  },
  anthropic: {
    provider: "anthropic",
    apiKeyPresent: false,
    generationModel: "claude-haiku-4-5",
    // Anthropic has no native embedding model.
    embeddingModel: undefined,
  },
  gemini: {
    provider: "gemini",
    apiKeyPresent: false,
    generationModel: "gemini-2.5-flash",
    embeddingModel: "text-embedding-004",
  },
  ollama: {
    provider: "ollama",
    apiKeyPresent: false,
    baseUrl: "http://localhost:11434",
    generationModel: "llama3.1:8b",
    embeddingModel: "nomic-embed-text",
  },
};

export const useAi = create<AiState>()(
  persist(
    (set) => ({
      providers: defaults,
      setProviderConfig: (p, patch) =>
        set((s) => ({
          providers: {
            ...s.providers,
            [p]: { ...s.providers[p], ...patch },
          },
        })),
      lastPrompt: "",
      setLastPrompt: (s) => set({ lastPrompt: s }),
    }),
    {
      name: "tusk-ai",
      storage: createJSONStorage(() => localStorage),
      // Hard-block raw key persistence even if a future contributor wires it up.
      partialize: (s) => ({
        providers: Object.fromEntries(
          Object.entries(s.providers).map(([k, v]) => [
            k,
            { ...v, apiKeyPresent: v.apiKeyPresent }, // shape preserved
          ]),
        ) as Record<AiProvider, ProviderConfig>,
        lastPrompt: s.lastPrompt,
      }),
    },
  ),
);
```

- [ ] **Step 3: Hydrate apiKeyPresent on app boot**

Edit `src/App.tsx`. Add an effect after the existing `useEffect` blocks:

```tsx
import { aiSecretListPresent } from "@/lib/keychain";
import { useAi } from "@/store/ai";

// inside App():
useEffect(() => {
  void aiSecretListPresent().then((present) => {
    const setProviderConfig = useAi.getState().setProviderConfig;
    (["openai", "anthropic", "gemini", "ollama"] as const).forEach((p) => {
      setProviderConfig(p, { apiKeyPresent: present.includes(p) });
    });
  });
}, []);
```

- [ ] **Step 4: Verify**

```bash
pnpm typecheck && pnpm lint
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/store/ai.ts src/store/settings.ts src/App.tsx
git commit -m "feat(week4): ai store + settings extension (providers, ragTopK, destructiveStrict)"
```

---

## Task 4: SettingsDialog shell + ProviderSection (key paste/save/delete)

**Goal:** A modal accessible from the sidebar/menu that hosts tabs (General/Providers/Schema Index/Advanced). Providers tab has 4 cards (one per provider) with API-key input, save, delete. Test button is a stub returning a toast (T6 wires real liveness).

**Files:**
- Create: `src/features/settings/SettingsDialog.tsx`
- Create: `src/features/settings/ProviderSection.tsx`
- Modify: `src/App.tsx` (open button + dialog mount)

**Steps:**

- [ ] **Step 1: Create `SettingsDialog.tsx`**

```tsx
import { useState } from "react";
import { Dialog as DialogPrimitive } from "radix-ui";

import { ProviderSection } from "./ProviderSection";

type Tab = "general" | "providers" | "schema-index" | "advanced";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialTab?: Tab;
}

export function SettingsDialog({ open, onOpenChange, initialTab }: Props) {
  const [tab, setTab] = useState<Tab>(initialTab ?? "providers");
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <DialogPrimitive.Content className="bg-card fixed top-1/2 left-1/2 z-50 flex h-[80vh] w-[720px] -translate-x-1/2 -translate-y-1/2 flex-col rounded border shadow">
          <DialogPrimitive.Title className="border-border border-b px-4 py-3 text-sm font-medium">
            Settings
          </DialogPrimitive.Title>
          <div className="flex flex-1 min-h-0">
            <nav className="border-border w-44 border-r p-2 text-xs">
              {(
                [
                  ["general", "General"],
                  ["providers", "Providers"],
                  ["schema-index", "Schema Index"],
                  ["advanced", "Advanced"],
                ] as const
              ).map(([k, label]) => (
                <button
                  key={k}
                  type="button"
                  onClick={() => setTab(k)}
                  className={`w-full rounded px-2 py-1 text-left ${
                    tab === k ? "bg-accent" : "hover:bg-accent/50"
                  }`}
                >
                  {label}
                </button>
              ))}
            </nav>
            <div className="flex-1 overflow-y-auto p-4">
              {tab === "providers" && <ProviderSection />}
              {tab === "general" && (
                <p className="text-muted-foreground text-xs">
                  General settings — coming later.
                </p>
              )}
              {tab === "schema-index" && (
                <p className="text-muted-foreground text-xs">
                  Schema index panel — see Task 16.
                </p>
              )}
              {tab === "advanced" && (
                <p className="text-muted-foreground text-xs">Reserved.</p>
              )}
            </div>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
```

- [ ] **Step 2: Create `ProviderSection.tsx`**

```tsx
import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  aiSecretSet,
  aiSecretDelete,
} from "@/lib/keychain";
import type { AiProvider } from "@/lib/types";
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
              setEnabledProviders(enabledProviders.filter((p) => p !== meta.id));
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
              setEnabledProviders(enabledProviders.filter((p) => p !== meta.id));
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
  config: import("@/lib/types").ProviderConfig;
  enabled: boolean;
  onToggle: (v: boolean) => void;
  onSave: (key: string) => Promise<void>;
  onDelete: () => Promise<void>;
  onConfigChange: (patch: Partial<import("@/lib/types").ProviderConfig>) => void;
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
          onChange={(e) => p.onConfigChange({ generationModel: e.target.value })}
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
```

- [ ] **Step 3: Mount dialog from App.tsx**

In `src/App.tsx`, add `import { Settings as SettingsIcon } from "lucide-react";` (alongside Moon/Sun) and `import { SettingsDialog } from "@/features/settings/SettingsDialog";`.

Inside `App()`:

```tsx
const [settingsOpen, setSettingsOpen] = useState(false);
```

Add a button next to the theme toggle in the sidebar header:

```tsx
<Button variant="ghost" size="icon-sm" onClick={() => setSettingsOpen(true)}>
  <SettingsIcon />
</Button>
```

Mount the dialog (next to `ConfirmModalHost`):

```tsx
<SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
```

- [ ] **Step 4: Verify**

```bash
pnpm typecheck && pnpm lint && pnpm format:check
```

Expected: PASS.

- [ ] **Step 5: Manual smoke**

```bash
pnpm tauri dev
```

Click the gear → Providers tab → paste a fake key into OpenAI → Save → key disappears + "key set" appears + checkbox auto-enabled. Remove → reverts. (Real liveness Test wired in T6.)

- [ ] **Step 6: Commit**

```bash
git add src/features/settings/SettingsDialog.tsx src/features/settings/ProviderSection.tsx src/App.tsx
git commit -m "feat(week4): SettingsDialog + ProviderSection (paste/save/delete keys, defaults, tools toggle)"
```

---

## Task 5: AI SDK provider factory + ModelPicker

**Goal:** A single function `buildModel(provider, modelId, key, baseUrl?)` returns a Vercel AI SDK 6 model instance for any of the 4 providers. Used by Cmd+K (Task 20) and the Test button (Task 6).

**Files:**
- Create: `src/lib/ai/providers.ts`
- Create: `src/features/settings/ModelPicker.tsx` (used in T20 — included now to avoid forward ref breakage)

**Steps:**

- [ ] **Step 1: Provider factory**

Create `src/lib/ai/providers.ts`:

```ts
import { createAnthropic } from "@ai-sdk/anthropic";
import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { createOpenAI } from "@ai-sdk/openai";
import { createOllama } from "ollama-ai-provider-v2";
import type { LanguageModel } from "ai";

import type { AiProvider } from "@/lib/types";

export interface BuildModelArgs {
  provider: AiProvider;
  modelId: string;
  apiKey: string;
  baseUrl?: string;
}

export function buildModel(args: BuildModelArgs): LanguageModel {
  switch (args.provider) {
    case "openai": {
      const oai = createOpenAI({ apiKey: args.apiKey });
      return oai(args.modelId);
    }
    case "anthropic": {
      const anth = createAnthropic({ apiKey: args.apiKey });
      return anth(args.modelId);
    }
    case "gemini": {
      const google = createGoogleGenerativeAI({ apiKey: args.apiKey });
      return google(args.modelId);
    }
    case "ollama": {
      const ollama = createOllama({
        baseURL: (args.baseUrl ?? "http://localhost:11434").replace(/\/$/, "") + "/api",
      });
      return ollama(args.modelId);
    }
  }
}

/** Suggested defaults for the model picker UI. */
export const DEFAULT_GENERATION_MODELS: Record<AiProvider, string[]> = {
  openai: ["gpt-4o-mini", "gpt-4o", "o4-mini"],
  anthropic: ["claude-haiku-4-5", "claude-sonnet-4-6", "claude-opus-4-7"],
  gemini: ["gemini-2.5-flash", "gemini-2.5-pro"],
  ollama: ["llama3.1:8b", "llama3.2", "qwen2.5:7b"],
};

export const DEFAULT_EMBEDDING_MODELS: Record<AiProvider, string | null> = {
  openai: "text-embedding-3-small",
  anthropic: null,
  gemini: "text-embedding-004",
  ollama: "nomic-embed-text",
};
```

- [ ] **Step 2: ModelPicker component**

Create `src/features/settings/ModelPicker.tsx`:

```tsx
import type { AiProvider } from "@/lib/types";
import { DEFAULT_GENERATION_MODELS } from "@/lib/ai/providers";

interface Props {
  provider: AiProvider;
  value: string;
  onChange: (v: string) => void;
}

export function ModelPicker({ provider, value, onChange }: Props) {
  const suggestions = DEFAULT_GENERATION_MODELS[provider];
  return (
    <div className="flex items-center gap-2">
      <input
        list={`models-${provider}`}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="border-input flex-1 rounded border px-2 py-1"
      />
      <datalist id={`models-${provider}`}>
        {suggestions.map((m) => (
          <option key={m} value={m} />
        ))}
      </datalist>
    </div>
  );
}
```

- [ ] **Step 3: Verify**

```bash
pnpm typecheck && pnpm build
```

Expected: PASS. The `ai` import paths must resolve; if the AI SDK 6 emits no `LanguageModel` type, fall back to `import type { LanguageModelV2 } from "ai"` — adjust the alias accordingly. (Subagent: try `LanguageModel` first; if `tsc` errors, replace with the type the SDK actually exports.)

- [ ] **Step 4: Commit**

```bash
git add src/lib/ai/providers.ts src/features/settings/ModelPicker.tsx
git commit -m "feat(week4): AI SDK provider factory + ModelPicker"
```

---

## Task 6: Provider liveness test button

**Goal:** Replace the stub in T4 with a real probe — short `generateText` ping that confirms the key + model + (for Ollama) baseUrl actually answer. Surface success/failure in toast.

**Files:**
- Modify: `src/features/settings/ProviderSection.tsx`
- Create: `src/lib/ai/probe.ts`

**Steps:**

- [ ] **Step 1: Probe helper**

Create `src/lib/ai/probe.ts`:

```ts
import { generateText } from "ai";

import { aiSecretGet } from "@/lib/keychain";
import { buildModel } from "@/lib/ai/providers";
import type { AiProvider } from "@/lib/types";

interface ProbeArgs {
  provider: AiProvider;
  modelId: string;
  baseUrl?: string;
}

export async function probeProvider(args: ProbeArgs): Promise<{
  ok: boolean;
  message: string;
}> {
  let apiKey: string | null = null;
  try {
    apiKey = await aiSecretGet(args.provider);
  } catch (e) {
    return { ok: false, message: `keychain: ${asMsg(e)}` };
  }
  if (apiKey === null && args.provider !== "ollama") {
    return { ok: false, message: "no key set" };
  }
  try {
    const model = buildModel({
      provider: args.provider,
      modelId: args.modelId,
      apiKey: apiKey ?? "",
      baseUrl: args.baseUrl,
    });
    const r = await generateText({
      model,
      prompt: 'Reply with the single word "pong".',
      // Keep cost trivial
      maxRetries: 0,
    });
    const text = (r.text ?? "").toLowerCase();
    return text.includes("pong")
      ? { ok: true, message: "pong" }
      : { ok: true, message: `responded: ${text.slice(0, 60)}` };
  } catch (e) {
    return { ok: false, message: asMsg(e) };
  } finally {
    apiKey = null;
  }
}

function asMsg(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}
```

- [ ] **Step 2: Wire Test button**

Edit `ProviderSection.tsx`. Replace the `onTest={() => toast("Test stub …")}` callback with:

```tsx
onTest={async () => {
  toast(`Testing ${meta.label}…`);
  const r = await probeProvider({
    provider: meta.id,
    modelId: providers[meta.id].generationModel,
    baseUrl: providers[meta.id].baseUrl,
  });
  if (r.ok) toast.success(`${meta.label}: ${r.message}`);
  else toast.error(`${meta.label}: ${r.message}`);
}}
```

Add the import: `import { probeProvider } from "@/lib/ai/probe";`.

- [ ] **Step 3: Verify**

```bash
pnpm typecheck && pnpm lint
```

Expected: PASS.

- [ ] **Step 4: Manual**

```bash
pnpm tauri dev
```

For each provider you have access to: paste real key, click Test, expect toast "OpenAI: pong" (or similar). With wrong key: error toast with provider message.

- [ ] **Step 5: Commit**

```bash
git add src/features/settings/ProviderSection.tsx src/lib/ai/probe.ts
git commit -m "feat(week4): provider Test button (live probe via generateText)"
```

---

## Task 7: Rust `classify_destructive` (AST) + extensive unit tests

**Goal:** Pure Rust function that takes SQL → `Vec<DestructiveFinding>`. AST-based, deterministic, multi-statement aware. No DB connection required.

**Files:**
- Create: `src-tauri/src/commands/destructive.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Steps:**

- [ ] **Step 1: Add module skeleton**

Create `src-tauri/src/commands/destructive.rs`:

```rust
//! AST-based destructive-statement classifier.
//!
//! The frontend `lib/ai/destructive.ts` is a regex pre-warning *only*; this
//! module is the single source of truth for the run gate.
//!
//! Wire format: `kind` is kebab-case to match the TypeScript `DestructiveKind`
//! union.

use serde::Serialize;
use sqlparser::ast::{
    AlterTableOperation, Expr, ObjectType, Statement,
};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DestructiveFinding {
    pub kind: DestructiveKind,
    pub statement_index: usize,
    pub message: String,
    pub affected_object: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DestructiveKind {
    DropDatabase,
    DropSchema,
    DropTable,
    DropColumn,
    DropIndex,
    DropView,
    DropFunction,
    Truncate,
    DeleteNoWhere,
    UpdateNoWhere,
    AlterDropConstraint,
    GrantRevokeAll,
    VacuumFull,
    ParserFailed,
}

pub fn classify_destructive(sql: &str) -> Vec<DestructiveFinding> {
    let stmts = match Parser::parse_sql(&PostgreSqlDialect {}, sql) {
        Ok(s) => s,
        Err(_) => {
            return vec![DestructiveFinding {
                kind: DestructiveKind::ParserFailed,
                statement_index: 0,
                message: "SQL could not be parsed; confirm before running"
                    .to_string(),
                affected_object: None,
            }]
        }
    };
    let mut out = Vec::new();
    for (i, stmt) in stmts.iter().enumerate() {
        if let Some(f) = classify_one(i, stmt) {
            out.push(f);
        }
    }
    out
}

fn classify_one(idx: usize, stmt: &Statement) -> Option<DestructiveFinding> {
    match stmt {
        Statement::Drop {
            object_type, names, ..
        } => {
            let names_str = names
                .iter()
                .map(object_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let kind = match object_type {
                ObjectType::Table => DestructiveKind::DropTable,
                ObjectType::Index => DestructiveKind::DropIndex,
                ObjectType::View => DestructiveKind::DropView,
                ObjectType::Schema => DestructiveKind::DropSchema,
                _ => DestructiveKind::DropTable, // 보수적
            };
            Some(DestructiveFinding {
                kind,
                statement_index: idx,
                message: format!("DROP {object_type:?} {names_str}"),
                affected_object: Some(names_str),
            })
        }
        Statement::Truncate { table_names, .. } => {
            let s = table_names
                .iter()
                .map(|t| object_to_string(&t.name))
                .collect::<Vec<_>>()
                .join(", ");
            Some(DestructiveFinding {
                kind: DestructiveKind::Truncate,
                statement_index: idx,
                message: format!("TRUNCATE will remove all rows from {s}"),
                affected_object: Some(s),
            })
        }
        Statement::Delete(d) => {
            let where_present = match &d.selection {
                Some(Expr::Value(_)) | None => false,
                Some(_) => true,
            };
            if !where_present {
                let target = d
                    .from
                    .iter()
                    .next()
                    .map(|t| match t {
                        sqlparser::ast::FromTable::WithFromKeyword(v)
                        | sqlparser::ast::FromTable::WithoutKeyword(v) => v
                            .first()
                            .map(|tw| match &tw.relation {
                                sqlparser::ast::TableFactor::Table { name, .. } => {
                                    object_to_string(name)
                                }
                                _ => "<unknown>".to_string(),
                            })
                            .unwrap_or_else(|| "<unknown>".to_string()),
                    })
                    .unwrap_or_else(|| "<unknown>".to_string());
                Some(DestructiveFinding {
                    kind: DestructiveKind::DeleteNoWhere,
                    statement_index: idx,
                    message: format!(
                        "DELETE without WHERE will remove all rows from {target}"
                    ),
                    affected_object: Some(target),
                })
            } else {
                None
            }
        }
        Statement::Update {
            table, selection, ..
        } => {
            if selection.is_none() {
                let target = match &table.relation {
                    sqlparser::ast::TableFactor::Table { name, .. } => {
                        object_to_string(name)
                    }
                    _ => "<unknown>".to_string(),
                };
                Some(DestructiveFinding {
                    kind: DestructiveKind::UpdateNoWhere,
                    statement_index: idx,
                    message: format!(
                        "UPDATE without WHERE will modify all rows in {target}"
                    ),
                    affected_object: Some(target),
                })
            } else {
                None
            }
        }
        Statement::AlterTable {
            name, operations, ..
        } => {
            for op in operations {
                match op {
                    AlterTableOperation::DropColumn { column_name, .. } => {
                        return Some(DestructiveFinding {
                            kind: DestructiveKind::DropColumn,
                            statement_index: idx,
                            message: format!(
                                "ALTER TABLE {} DROP COLUMN {} will remove the column and its data",
                                object_to_string(name),
                                column_name.value
                            ),
                            affected_object: Some(format!(
                                "{}.{}",
                                object_to_string(name),
                                column_name.value
                            )),
                        });
                    }
                    AlterTableOperation::DropConstraint { name: c, .. } => {
                        return Some(DestructiveFinding {
                            kind: DestructiveKind::AlterDropConstraint,
                            statement_index: idx,
                            message: format!(
                                "ALTER TABLE {} DROP CONSTRAINT {}",
                                object_to_string(name),
                                c.value
                            ),
                            affected_object: Some(object_to_string(name)),
                        });
                    }
                    _ => {}
                }
            }
            None
        }
        Statement::Grant { privileges, .. } | Statement::Revoke { privileges, .. } => {
            let all = matches!(privileges, sqlparser::ast::Privileges::All { .. });
            if all {
                Some(DestructiveFinding {
                    kind: DestructiveKind::GrantRevokeAll,
                    statement_index: idx,
                    message: "GRANT/REVOKE ALL changes broad privileges".into(),
                    affected_object: None,
                })
            } else {
                None
            }
        }
        Statement::Vacuum { .. } => {
            // sqlparser 0.52에서 옵션 노출이 제한적 — SQL 문자열에서 "FULL" 토큰만 검사하는 보조 로직.
            // 여기서는 보수적으로 통과시키고, 호출자가 raw SQL로 "VACUUM FULL" 검사 한 번 더.
            None
        }
        _ => None,
    }
}

fn object_to_string(name: &sqlparser::ast::ObjectName) -> String {
    name.0
        .iter()
        .map(|i| i.value.clone())
        .collect::<Vec<_>>()
        .join(".")
}

/// VACUUM FULL은 sqlparser AST에서 직접 분기하기 까다로워 raw 토큰으로 보조 검사.
pub fn classify_vacuum_full(sql: &str) -> Vec<DestructiveFinding> {
    let mut out = Vec::new();
    let upper = sql.to_uppercase();
    if upper.contains("VACUUM FULL") {
        out.push(DestructiveFinding {
            kind: DestructiveKind::VacuumFull,
            statement_index: 0,
            message: "VACUUM FULL takes an exclusive lock and rewrites the table".into(),
            affected_object: None,
        });
    }
    out
}

pub fn classify_all(sql: &str) -> Vec<DestructiveFinding> {
    let mut out = classify_destructive(sql);
    out.extend(classify_vacuum_full(sql));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(sql: &str) -> Vec<DestructiveKind> {
        classify_all(sql).into_iter().map(|f| f.kind).collect()
    }

    #[test]
    fn drop_table() {
        assert_eq!(kinds("DROP TABLE users"), vec![DestructiveKind::DropTable]);
    }

    #[test]
    fn drop_schema_cascade() {
        assert_eq!(
            kinds("DROP SCHEMA app CASCADE"),
            vec![DestructiveKind::DropSchema]
        );
    }

    #[test]
    fn drop_index_view_function() {
        assert_eq!(kinds("DROP INDEX idx_a"), vec![DestructiveKind::DropIndex]);
        assert_eq!(kinds("DROP VIEW v"), vec![DestructiveKind::DropView]);
    }

    #[test]
    fn truncate_named_table() {
        assert_eq!(
            kinds("TRUNCATE TABLE public.audit_log"),
            vec![DestructiveKind::Truncate]
        );
    }

    #[test]
    fn delete_without_where() {
        assert_eq!(
            kinds("DELETE FROM users"),
            vec![DestructiveKind::DeleteNoWhere]
        );
    }

    #[test]
    fn delete_with_where_is_safe() {
        assert!(kinds("DELETE FROM users WHERE id = 1").is_empty());
    }

    #[test]
    fn update_without_where() {
        assert_eq!(
            kinds("UPDATE users SET active = false"),
            vec![DestructiveKind::UpdateNoWhere]
        );
    }

    #[test]
    fn update_with_where_is_safe() {
        assert!(kinds("UPDATE users SET active = false WHERE id = 1").is_empty());
    }

    #[test]
    fn alter_drop_column() {
        assert_eq!(
            kinds("ALTER TABLE users DROP COLUMN email"),
            vec![DestructiveKind::DropColumn]
        );
    }

    #[test]
    fn alter_drop_constraint() {
        assert_eq!(
            kinds("ALTER TABLE users DROP CONSTRAINT users_pkey"),
            vec![DestructiveKind::AlterDropConstraint]
        );
    }

    #[test]
    fn grant_all_privileges() {
        assert_eq!(
            kinds("GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO bob"),
            vec![DestructiveKind::GrantRevokeAll]
        );
    }

    #[test]
    fn revoke_all_privileges() {
        assert_eq!(
            kinds("REVOKE ALL PRIVILEGES ON DATABASE app FROM bob"),
            vec![DestructiveKind::GrantRevokeAll]
        );
    }

    #[test]
    fn vacuum_full_token_match() {
        assert_eq!(kinds("VACUUM FULL users"), vec![DestructiveKind::VacuumFull]);
    }

    #[test]
    fn select_is_safe() {
        assert!(kinds("SELECT * FROM users").is_empty());
    }

    #[test]
    fn unparseable_returns_parser_failed() {
        let r = classify_destructive("this is not sql");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, DestructiveKind::ParserFailed);
    }

    #[test]
    fn multi_statement_collects_each_finding() {
        let sql = "DELETE FROM a; UPDATE b SET x=1; DROP TABLE c";
        let r = classify_destructive(sql);
        let kinds: Vec<_> = r.iter().map(|f| f.kind).collect();
        assert_eq!(
            kinds,
            vec![
                DestructiveKind::DeleteNoWhere,
                DestructiveKind::UpdateNoWhere,
                DestructiveKind::DropTable,
            ]
        );
        assert_eq!(r[0].statement_index, 0);
        assert_eq!(r[1].statement_index, 1);
        assert_eq!(r[2].statement_index, 2);
    }
}
```

- [ ] **Step 2: Wire module**

Edit `src-tauri/src/commands/mod.rs`: add `pub mod destructive;`.

- [ ] **Step 3: Run all unit tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml destructive::tests
```

Expected: all green. Where sqlparser AST shape disagrees with the snippet (the crate evolves), the subagent must consult the version pinned in `Cargo.toml` and adjust pattern matches. Tests must remain unchanged.

- [ ] **Step 4: Lint/fmt**

```bash
pnpm rust:lint && pnpm rust:fmt:check
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/destructive.rs src-tauri/src/commands/mod.rs
git commit -m "feat(week4): destructive statement classifier (AST) + unit tests"
```

---

## Task 8: Expose `classify_destructive` to frontend + regex pre-warn mirror

**Goal:** A Tauri command that returns `Vec<DestructiveFinding>` for an arbitrary SQL string, plus a small frontend regex helper for instant in-typing warning (UX only — never the gate).

**Files:**
- Modify: `src-tauri/src/commands/destructive.rs` (add `#[tauri::command]`)
- Modify: `src-tauri/src/lib.rs`
- Create: `src/lib/ai/destructive.ts`

**Steps:**

- [ ] **Step 1: Add tauri command**

Append to `src-tauri/src/commands/destructive.rs`:

```rust
#[tauri::command]
pub fn classify_destructive_sql(sql: String) -> Vec<DestructiveFinding> {
    classify_all(&sql)
}
```

- [ ] **Step 2: Register**

Edit `src-tauri/src/lib.rs` `invoke_handler!`:

```rust
            commands::destructive::classify_destructive_sql,
```

- [ ] **Step 3: Frontend pre-warn helper**

Create `src/lib/ai/destructive.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

import type { DestructiveFinding } from "@/lib/types";

const FAST_PATTERNS: { kind: DestructiveFinding["kind"]; re: RegExp }[] = [
  { kind: "drop-table", re: /\bdrop\s+table\b/i },
  { kind: "drop-schema", re: /\bdrop\s+schema\b/i },
  { kind: "drop-database", re: /\bdrop\s+database\b/i },
  { kind: "drop-view", re: /\bdrop\s+view\b/i },
  { kind: "drop-index", re: /\bdrop\s+index\b/i },
  { kind: "drop-function", re: /\bdrop\s+function\b/i },
  { kind: "truncate", re: /\btruncate\b/i },
  { kind: "vacuum-full", re: /\bvacuum\s+full\b/i },
  { kind: "alter-drop-constraint", re: /\bdrop\s+constraint\b/i },
  { kind: "drop-column", re: /\bdrop\s+column\b/i },
  // DELETE / UPDATE without WHERE는 정규식만으로 정확히 못 잡음 — false positive 감수.
  { kind: "delete-no-where", re: /\bdelete\s+from\b(?![\s\S]*\bwhere\b)/i },
  { kind: "update-no-where", re: /\bupdate\s+\S+\s+set\b(?![\s\S]*\bwhere\b)/i },
  { kind: "grant-revoke-all", re: /\b(grant|revoke)\s+all\b/i },
];

/** Pre-warn — fast regex, may have false positives. Never use as a gate. */
export function fastDestructiveWarn(sql: string): DestructiveFinding["kind"][] {
  return FAST_PATTERNS.filter((p) => p.re.test(sql)).map((p) => p.kind);
}

/** Authoritative gate — calls the Rust AST classifier. */
export async function classifyDestructive(
  sql: string,
): Promise<DestructiveFinding[]> {
  return invoke<DestructiveFinding[]>("classify_destructive_sql", { sql });
}
```

- [ ] **Step 4: Vitest for fast pre-warn**

Create `src/lib/ai/destructive.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { fastDestructiveWarn } from "./destructive";

describe("fastDestructiveWarn", () => {
  it("flags DROP TABLE", () => {
    expect(fastDestructiveWarn("DROP TABLE users")).toContain("drop-table");
  });
  it("flags TRUNCATE", () => {
    expect(fastDestructiveWarn("truncate audit_log")).toContain("truncate");
  });
  it("does not flag DELETE with WHERE", () => {
    const r = fastDestructiveWarn("DELETE FROM users WHERE id = 1");
    expect(r).not.toContain("delete-no-where");
  });
  it("flags DELETE without WHERE", () => {
    const r = fastDestructiveWarn("DELETE FROM users");
    expect(r).toContain("delete-no-where");
  });
  it("flags VACUUM FULL", () => {
    expect(fastDestructiveWarn("VACUUM FULL users")).toContain("vacuum-full");
  });
  it("returns empty for SELECT", () => {
    expect(fastDestructiveWarn("SELECT * FROM users")).toEqual([]);
  });
});
```

- [ ] **Step 5: Run vitest**

```bash
pnpm test
```

Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/destructive.rs src-tauri/src/lib.rs src/lib/ai/destructive.ts src/lib/ai/destructive.test.ts
git commit -m "feat(week4): classify_destructive_sql command + frontend regex pre-warn"
```

---

## Task 9: DestructiveModal (Standard + Strict)

**Goal:** Modal that takes a list of findings + the SQL preview and yields a boolean (run / cancel). Standard mode: two-button. Strict: typed keyword.

**Files:**
- Create: `src/features/ai/DestructiveModal.tsx`
- Create: `src/features/ai/DestructiveModal.test.tsx`
- Modify: `src/lib/confirm.tsx` (add a typed-confirm variant) — or expose a new helper instead.

**Steps:**

- [ ] **Step 1: Decide API shape**

Use a one-shot `confirmDestructive(opts) → Promise<boolean>` instead of bolting onto `openConfirmModal` (different contract — strict mode requires extra UI state).

Create `src/features/ai/DestructiveModal.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";
import { Dialog as DialogPrimitive } from "radix-ui";
import { AlertTriangle } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { DestructiveFinding } from "@/lib/types";

interface PendingRequest {
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
    pending = { ...opts, resolve };
    listener?.(pending);
  });
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

  const requiredKeyword = useMemo(() => {
    if (!req) return "";
    // first destructive statement's first SQL keyword (DROP/TRUNCATE/DELETE/UPDATE/ALTER/GRANT/REVOKE/VACUUM)
    const first = req.findings[0];
    if (!first) return "";
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
      candidates.find((c) =>
        req.sql.toUpperCase().includes(c),
      ) ?? "CONFIRM"
    );
  }, [req]);

  const [typed, setTyped] = useState("");
  useEffect(() => setTyped(""), [req]);

  if (!req) return null;
  const canRun = !req.strict || typed.trim().toUpperCase() === requiredKeyword;
  return (
    <DialogPrimitive.Root open onOpenChange={(o) => !o && close(false)}>
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
            <Button variant="ghost" onClick={() => close(false)}>
              Cancel
            </Button>
            <Button
              disabled={!canRun}
              onClick={() => close(true)}
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
```

- [ ] **Step 2: Mount host in App.tsx**

In `src/App.tsx` add `import { DestructiveModalHost } from "@/features/ai/DestructiveModal";` and render `<DestructiveModalHost />` next to `<ConfirmModalHost />`.

- [ ] **Step 3: Vitest**

Create `src/features/ai/DestructiveModal.test.tsx`:

```tsx
import { afterEach, describe, expect, it } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";

import {
  DestructiveModalHost,
  confirmDestructive,
} from "./DestructiveModal";

afterEach(cleanup);

describe("DestructiveModal", () => {
  it("standard mode: clicking Run anyway resolves true", async () => {
    render(<DestructiveModalHost />);
    const promise = confirmDestructive({
      findings: [
        {
          kind: "drop-table",
          statementIndex: 0,
          message: "DROP TABLE foo",
          affectedObject: "foo",
        },
      ],
      sql: "DROP TABLE foo",
      strict: false,
    });
    // Dialog renders async — wait one tick.
    await screen.findByText(/Confirm destructive/);
    fireEvent.click(screen.getByText(/Run anyway/));
    expect(await promise).toBe(true);
  });

  it("standard mode: clicking Cancel resolves false", async () => {
    render(<DestructiveModalHost />);
    const promise = confirmDestructive({
      findings: [
        { kind: "truncate", statementIndex: 0, message: "...", affectedObject: "x" },
      ],
      sql: "TRUNCATE x",
      strict: false,
    });
    await screen.findByText(/Confirm destructive/);
    fireEvent.click(screen.getByText("Cancel"));
    expect(await promise).toBe(false);
  });

  it("strict mode: Run disabled until keyword typed", async () => {
    render(<DestructiveModalHost />);
    const promise = confirmDestructive({
      findings: [
        { kind: "drop-table", statementIndex: 0, message: "...", affectedObject: "x" },
      ],
      sql: "DROP TABLE x",
      strict: true,
    });
    await screen.findByText(/Confirm destructive/);
    const runBtn = screen.getByText("Run") as HTMLButtonElement;
    expect(runBtn.disabled).toBe(true);
    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "DROP" },
    });
    expect((screen.getByText("Run") as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByText("Run"));
    expect(await promise).toBe(true);
  });
});
```

- [ ] **Step 4: Run vitest**

```bash
pnpm test
```

Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/features/ai/DestructiveModal.tsx src/features/ai/DestructiveModal.test.tsx src/App.tsx
git commit -m "feat(week4): DestructiveModal (Standard / Strict) + vitest"
```

---

## Task 10: Integrate destructive guard into execute_query path

**Goal:** Every Run path (`Cmd+Enter` in editor, palette re-run, future Cmd+K Apply) goes through `classifyDestructive` first; if findings exist, present `confirmDestructive` and short-circuit on cancel. The Rust side does NOT block on destructive in v1 — UX is the gate.

**Files:**
- Modify: `src/features/editor/EditorPane.tsx` (Cmd+Enter `run` callback)
- Modify: `src/features/history/HistoryPalette.tsx` (palette re-run)
- Create: `src/lib/ai/runGate.ts` (shared helper)

**Steps:**

- [ ] **Step 1: Run gate helper**

Create `src/lib/ai/runGate.ts`:

```ts
import { classifyDestructive } from "@/lib/ai/destructive";
import { confirmDestructive } from "@/features/ai/DestructiveModal";
import { useSettings } from "@/store/settings";

/** Returns true when execution may proceed, false when user cancelled. */
export async function runGate(sql: string): Promise<boolean> {
  const findings = await classifyDestructive(sql);
  if (findings.length === 0) return true;
  const strict = useSettings.getState().destructiveStrict;
  return confirmDestructive({ findings, sql, strict });
}
```

- [ ] **Step 2: Patch EditorPane.run**

In `src/features/editor/EditorPane.tsx`, before `executeQuery(...)`:

```ts
const proceed = await runGate(sqlToRun);
if (!proceed) {
  setBusy(activeTab.id, false);
  return;
}
```

Add `import { runGate } from "@/lib/ai/runGate";`.

- [ ] **Step 3: Patch HistoryPalette pick**

In `src/features/history/HistoryPalette.tsx`, on the "execute" path (if the palette has a re-run button — otherwise loading SQL into editor and letting the user press Cmd+Enter is enough). For v1, palette only loads SQL into the active tab; the user still presses Cmd+Enter, so EditorPane.run already gates it. **No edit needed in HistoryPalette.tsx if it doesn't auto-execute.** Document this in the plan and skip.

- [ ] **Step 4: Vitest for runGate**

Create `src/lib/ai/runGate.test.ts`:

```ts
import { afterEach, describe, expect, it, vi } from "vitest";

import { runGate } from "./runGate";

vi.mock("@/lib/ai/destructive", () => ({
  classifyDestructive: vi.fn(),
}));
vi.mock("@/features/ai/DestructiveModal", () => ({
  confirmDestructive: vi.fn(),
}));
vi.mock("@/store/settings", () => ({
  useSettings: { getState: () => ({ destructiveStrict: false }) },
}));

import { classifyDestructive } from "@/lib/ai/destructive";
import { confirmDestructive } from "@/features/ai/DestructiveModal";

afterEach(() => vi.clearAllMocks());

describe("runGate", () => {
  it("returns true with no findings", async () => {
    (classifyDestructive as unknown as vi.Mock).mockResolvedValue([]);
    expect(await runGate("SELECT 1")).toBe(true);
    expect(confirmDestructive).not.toHaveBeenCalled();
  });

  it("delegates to confirmDestructive when findings exist", async () => {
    (classifyDestructive as unknown as vi.Mock).mockResolvedValue([
      { kind: "drop-table", statementIndex: 0, message: "x" },
    ]);
    (confirmDestructive as unknown as vi.Mock).mockResolvedValue(true);
    expect(await runGate("DROP TABLE x")).toBe(true);
    expect(confirmDestructive).toHaveBeenCalledOnce();
  });

  it("returns false on cancel", async () => {
    (classifyDestructive as unknown as vi.Mock).mockResolvedValue([
      { kind: "truncate", statementIndex: 0, message: "x" },
    ]);
    (confirmDestructive as unknown as vi.Mock).mockResolvedValue(false);
    expect(await runGate("TRUNCATE x")).toBe(false);
  });
});
```

- [ ] **Step 5: Run vitest**

```bash
pnpm test
```

Expected: green.

- [ ] **Step 6: Manual smoke**

```bash
pnpm tauri dev
```

Type `DROP TABLE foo;` in editor → Cmd+Enter → modal → Cancel = nothing happens, Run anyway = query executes (likely DB error if table absent, expected).

- [ ] **Step 7: Commit**

```bash
git add src/lib/ai/runGate.ts src/lib/ai/runGate.test.ts src/features/editor/EditorPane.tsx
git commit -m "feat(week4): runGate — every execute path gated by destructive AST"
```

---

## Task 11: Migration `003_ai` — schema_embedding + ai_history tables

**Goal:** Add SQLite tables for embedding BLOBs and AI metadata. Reuse Week 3's history_entry FK.

**Files:**
- Modify: `src-tauri/src/db/state.rs` (add migration step)

**Steps:**

- [ ] **Step 1: Find the migration runner**

Open `src-tauri/src/db/state.rs`. Identify the migration list (Week 3 introduced `migration 002_history`). Append a third migration entry pointing to a new SQL string.

- [ ] **Step 2: Add migration SQL**

Add a constant near the existing migrations:

```rust
const MIGRATION_003_AI: &str = "
CREATE TABLE schema_embedding (
    id              TEXT PRIMARY KEY,
    conn_id         TEXT NOT NULL,
    schema_name     TEXT NOT NULL,
    table_name      TEXT NOT NULL,
    pg_relid        INTEGER NOT NULL,
    ddl_checksum    TEXT NOT NULL,
    embedding       BLOB NOT NULL,
    embedding_dim   INTEGER NOT NULL,
    embedding_model TEXT NOT NULL,
    embedded_at     INTEGER NOT NULL,
    UNIQUE (conn_id, schema_name, table_name)
);
CREATE INDEX idx_schema_embedding_conn ON schema_embedding(conn_id);

CREATE TABLE ai_history (
    entry_id          TEXT PRIMARY KEY REFERENCES history_entry(id) ON DELETE CASCADE,
    provider          TEXT NOT NULL,
    generation_model  TEXT NOT NULL,
    embedding_model   TEXT,
    prompt            TEXT NOT NULL,
    generated_sql     TEXT NOT NULL,
    top_k_tables      TEXT NOT NULL,
    tool_calls        TEXT,
    prompt_tokens     INTEGER,
    completion_tokens INTEGER
);
";
```

(NB: column named `schema_name` to avoid the SQLite keyword `SCHEMA` confusion in some contexts. Match this naming in queries.)

- [ ] **Step 3: Wire into the migrations array**

Find the `migrations: &[(...)]` array and add `("003_ai", MIGRATION_003_AI)` (matching the project's existing tuple shape).

- [ ] **Step 4: Run startup test**

```bash
cargo test --manifest-path src-tauri/Cargo.toml state::tests
```

Existing migration test should pass (migrations are idempotent / version-tracked). If a "schema versions" sanity test exists, it picks up the new entry automatically.

- [ ] **Step 5: Manual reset for dev**

If the dev sqlite at `~/Library/Application Support/dev.tusk.app/tusk.db` already has version 002 applied, delete it once or rely on the migration runner — check actual mechanism in state.rs and follow it.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db/state.rs
git commit -m "feat(week4): migration 003_ai — schema_embedding + ai_history"
```

---

## Task 12: `db/schema_embed.rs` — build_table_ddl + checksum

**Goal:** Pure-ish builder that synthesizes a `CREATE TABLE` string from PG catalog rows. Stable output (deterministic ordering) for cheap checksumming.

**Files:**
- Create: `src-tauri/src/db/schema_embed.rs`
- Modify: `src-tauri/src/db/mod.rs`

**Steps:**

- [ ] **Step 1: Create module**

```rust
//! Synthesize CREATE TABLE DDL strings from `pg_catalog` rows.
//! Deterministic — same input always yields same string + checksum.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::Serialize;
use sqlx::{PgPool, Row};

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Serialize)]
pub struct TableDdl {
    pub schema: String,
    pub table: String,
    pub pg_relid: u32,
    pub ddl: String,
    pub checksum: String,
}

pub async fn list_user_tables(pool: &PgPool) -> TuskResult<Vec<(String, String, u32)>> {
    let rows = sqlx::query(
        "SELECT n.nspname, c.relname, c.oid::int4
         FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.relkind IN ('r','p','m')
           AND n.nspname NOT IN ('pg_catalog','information_schema','pg_toast')
         ORDER BY n.nspname, c.relname",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let n: String = r.try_get(0).map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        let t: String = r.try_get(1).map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        let oid: i32 = r.try_get(2).map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        out.push((n, t, oid as u32));
    }
    Ok(out)
}

pub async fn build_table_ddl(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> TuskResult<TableDdl> {
    // Columns
    let cols = sqlx::query(
        "SELECT a.attname, format_type(a.atttypid, a.atttypmod), a.attnotnull,
                pg_get_expr(d.adbin, d.adrelid),
                col_description(a.attrelid, a.attnum)
         FROM pg_attribute a
         LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
         WHERE a.attrelid = ($1 || '.' || $2)::regclass
           AND a.attnum > 0
           AND NOT a.attisdropped
         ORDER BY a.attnum",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;

    let mut ddl = format!("CREATE TABLE \"{schema}\".\"{table}\" (\n");
    for (i, r) in cols.iter().enumerate() {
        let name: String = r.try_get(0).map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        let ty: String = r.try_get(1).map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        let notnull: bool = r.try_get(2).map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
        let default: Option<String> = r.try_get(3).ok();
        let comment: Option<String> = r.try_get(4).ok().flatten();
        let mut line = format!("  \"{name}\" {ty}");
        if notnull { line.push_str(" NOT NULL"); }
        if let Some(d) = default { line.push_str(&format!(" DEFAULT {d}")); }
        if i + 1 < cols.len() { line.push(','); }
        if let Some(c) = comment { line.push_str(&format!("  -- {c}")); }
        line.push('\n');
        ddl.push_str(&line);
    }
    ddl.push_str(");\n");

    // Primary key
    if let Ok(pk_rows) = sqlx::query(
        "SELECT a.attname
         FROM pg_index i
         JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
         WHERE i.indrelid = ($1 || '.' || $2)::regclass AND i.indisprimary
         ORDER BY array_position(i.indkey, a.attnum)",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    {
        let pk: Vec<String> = pk_rows
            .iter()
            .filter_map(|r| r.try_get::<String, _>(0).ok())
            .collect();
        if !pk.is_empty() {
            ddl.push_str(&format!(
                "ALTER TABLE \"{schema}\".\"{table}\" ADD PRIMARY KEY ({});\n",
                pk.iter()
                    .map(|c| format!("\"{c}\""))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
    }

    // Foreign keys (kept short — name + columns + ref)
    if let Ok(fk_rows) = sqlx::query(
        "SELECT conname, pg_get_constraintdef(oid)
         FROM pg_constraint
         WHERE conrelid = ($1 || '.' || $2)::regclass AND contype = 'f'
         ORDER BY conname",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    {
        for r in fk_rows {
            let name: String = r.try_get(0).unwrap_or_default();
            let def: String = r.try_get(1).unwrap_or_default();
            ddl.push_str(&format!(
                "ALTER TABLE \"{schema}\".\"{table}\" ADD CONSTRAINT {name} {def};\n"
            ));
        }
    }

    // Table comment
    if let Ok(c) = sqlx::query(
        "SELECT obj_description(($1 || '.' || $2)::regclass, 'pg_class')",
    )
    .bind(schema)
    .bind(table)
    .fetch_one(pool)
    .await
    {
        if let Ok(Some(comment)) = c.try_get::<Option<String>, _>(0) {
            ddl.push_str(&format!(
                "COMMENT ON TABLE \"{schema}\".\"{table}\" IS '{}';\n",
                comment.replace('\'', "''")
            ));
        }
    }

    let oid_row = sqlx::query("SELECT ($1 || '.' || $2)::regclass::oid::int4")
        .bind(schema)
        .bind(table)
        .fetch_one(pool)
        .await
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
    let pg_relid: i32 = oid_row
        .try_get(0)
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;

    let mut h = DefaultHasher::new();
    ddl.hash(&mut h);
    let checksum = format!("{:016x}", h.finish());

    Ok(TableDdl {
        schema: schema.to_string(),
        table: table.to_string(),
        pg_relid: pg_relid as u32,
        ddl,
        checksum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_is_stable_for_same_string() {
        let a = "CREATE TABLE x (id int);";
        let mut h1 = DefaultHasher::new();
        a.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        a.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }
}
```

- [ ] **Step 2: Register module**

In `src-tauri/src/db/mod.rs` add `pub mod schema_embed;`.

- [ ] **Step 3: Compile**

```bash
pnpm rust:check && pnpm rust:lint
```

Expected: PASS.

- [ ] **Step 4: Integration test (postgres required)**

Append to `src-tauri/tests/schema_index.rs` (will be created in T15) — for now just rely on compile + future T15 covering it.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/schema_embed.rs src-tauri/src/db/mod.rs
git commit -m "feat(week4): build_table_ddl — synthesize CREATE TABLE DDL from pg_catalog"
```

---

## Task 13: `db/embedding_store.rs` — BLOB I/O + cosine top_k

**Goal:** Read/write `f32[]` BLOBs in `schema_embedding`, plus an in-memory cosine top-K. No DB connection (rusqlite) inside the cosine — pure compute.

**Files:**
- Create: `src-tauri/src/db/embedding_store.rs`
- Modify: `src-tauri/src/db/mod.rs`

**Steps:**

- [ ] **Step 1: Create module**

```rust
//! BLOB persistence for table embeddings + in-memory cosine top-K.

use std::cmp::Ordering;

use rusqlite::params;
use serde::Serialize;
use uuid::Uuid;

use crate::db::state::StateStore;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEmbedding {
    pub schema: String,
    pub table: String,
    pub embedding: Vec<f32>,
    pub embedding_dim: usize,
    pub embedding_model: String,
}

pub fn upsert_embedding(
    store: &StateStore,
    conn_id: &str,
    schema: &str,
    table: &str,
    pg_relid: u32,
    ddl_checksum: &str,
    embedding: &[f32],
    embedding_model: &str,
    embedded_at: i64,
) -> TuskResult<()> {
    let bytes: &[u8] = bytemuck::cast_slice(embedding);
    let dim = embedding.len() as i64;
    let id = Uuid::new_v4().to_string();
    let conn = store.lock();
    conn.execute(
        "INSERT INTO schema_embedding
         (id, conn_id, schema_name, table_name, pg_relid, ddl_checksum,
          embedding, embedding_dim, embedding_model, embedded_at)
         VALUES (?,?,?,?,?,?,?,?,?,?)
         ON CONFLICT(conn_id, schema_name, table_name)
         DO UPDATE SET
           pg_relid = excluded.pg_relid,
           ddl_checksum = excluded.ddl_checksum,
           embedding = excluded.embedding,
           embedding_dim = excluded.embedding_dim,
           embedding_model = excluded.embedding_model,
           embedded_at = excluded.embedded_at",
        params![
            id,
            conn_id,
            schema,
            table,
            pg_relid as i64,
            ddl_checksum,
            bytes,
            dim,
            embedding_model,
            embedded_at
        ],
    )
    .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
    Ok(())
}

pub fn lookup_one(
    store: &StateStore,
    conn_id: &str,
    schema: &str,
    table: &str,
) -> TuskResult<Option<(u32, String, String)>> {
    let conn = store.lock();
    let mut stmt = conn
        .prepare(
            "SELECT pg_relid, ddl_checksum, embedding_model
             FROM schema_embedding
             WHERE conn_id = ? AND schema_name = ? AND table_name = ?",
        )
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
    let r = stmt
        .query_row(params![conn_id, schema, table], |r| {
            Ok((
                r.get::<_, i64>(0)? as u32,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .ok();
    Ok(r)
}

pub fn load_all(
    store: &StateStore,
    conn_id: &str,
) -> TuskResult<Vec<StoredEmbedding>> {
    let conn = store.lock();
    let mut stmt = conn
        .prepare(
            "SELECT schema_name, table_name, embedding, embedding_dim, embedding_model
             FROM schema_embedding
             WHERE conn_id = ?",
        )
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
    let rows = stmt
        .query_map(params![conn_id], |r| {
            let blob: Vec<u8> = r.get(2)?;
            let dim: i64 = r.get(3)?;
            let model: String = r.get(4)?;
            let schema: String = r.get(0)?;
            let table: String = r.get(1)?;
            let f: &[f32] = bytemuck::cast_slice(&blob);
            Ok(StoredEmbedding {
                schema,
                table,
                embedding: f.to_vec(),
                embedding_dim: dim as usize,
                embedding_model: model,
            })
        })
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| TuskError::SchemaIndex(e.to_string()))?);
    }
    Ok(out)
}

pub fn delete_for_conn(store: &StateStore, conn_id: &str) -> TuskResult<()> {
    store
        .lock()
        .execute(
            "DELETE FROM schema_embedding WHERE conn_id = ?",
            params![conn_id],
        )
        .map(|_| ())
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredTable {
    pub schema: String,
    pub table: String,
    pub similarity: f32,
}

pub fn cosine_top_k(
    query: &[f32],
    rows: &[StoredEmbedding],
    k: usize,
) -> Vec<ScoredTable> {
    let q_norm = norm(query);
    if q_norm == 0.0 {
        return Vec::new();
    }
    let mut scored: Vec<ScoredTable> = rows
        .iter()
        .filter(|r| r.embedding.len() == query.len())
        .map(|r| {
            let dot: f32 = r
                .embedding
                .iter()
                .zip(query.iter())
                .map(|(a, b)| a * b)
                .sum();
            let n = norm(&r.embedding);
            let sim = if n == 0.0 { 0.0 } else { dot / (q_norm * n) };
            ScoredTable {
                schema: r.schema.clone(),
                table: r.table.clone(),
                similarity: sim,
            }
        })
        .collect();
    scored.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(Ordering::Equal)
    });
    scored.truncate(k);
    scored
}

fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emb(s: &str, t: &str, v: Vec<f32>) -> StoredEmbedding {
        StoredEmbedding {
            schema: s.into(),
            table: t.into(),
            embedding_dim: v.len(),
            embedding: v,
            embedding_model: "test".into(),
        }
    }

    #[test]
    fn cosine_returns_most_similar_first() {
        let q = vec![1.0, 0.0, 0.0];
        let rows = vec![
            emb("a", "x", vec![0.0, 1.0, 0.0]),     // orthogonal
            emb("a", "y", vec![1.0, 0.0, 0.0]),     // identical
            emb("a", "z", vec![0.9, 0.1, 0.0]),     // close
        ];
        let r = cosine_top_k(&q, &rows, 2);
        assert_eq!(r[0].table, "y");
        assert_eq!(r[1].table, "z");
    }

    #[test]
    fn cosine_skips_dim_mismatch() {
        let q = vec![1.0, 0.0];
        let rows = vec![
            emb("a", "ok", vec![1.0, 0.0]),
            emb("a", "bad", vec![1.0, 0.0, 0.0]),
        ];
        let r = cosine_top_k(&q, &rows, 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].table, "ok");
    }

    #[test]
    fn cosine_zero_vector_query_returns_empty() {
        let q = vec![0.0, 0.0];
        let rows = vec![emb("a", "ok", vec![1.0, 0.0])];
        assert!(cosine_top_k(&q, &rows, 5).is_empty());
    }
}
```

> **NB:** the existing `StateStore` exposes `db: Mutex<Sqlite>`. The snippet uses `.lock()` — adjust to whatever method name the codebase uses (e.g., `store.db.lock().unwrap()` or a helper `with_conn(|c| ...)`). The subagent must read `db/state.rs` and call the established API; do not silently change `state.rs`.

- [ ] **Step 2: Register module**

In `src-tauri/src/db/mod.rs` add `pub mod embedding_store;`.

- [ ] **Step 3: Run unit tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml embedding_store::tests
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/embedding_store.rs src-tauri/src/db/mod.rs
git commit -m "feat(week4): embedding_store — BLOB upsert + cosine top_k"
```

---

## Task 14: Embedding HTTP adapters (OpenAI / Gemini / Ollama)

**Goal:** A `embed_one(provider, model, base_url?, api_key, text) -> Vec<f32>` Rust function. Used by the schema-sync command (T15) for each table DDL and (later) by `schema_top_k` for the user prompt.

**Files:**
- Create: `src-tauri/src/db/embedding_http.rs`
- Modify: `src-tauri/src/db/mod.rs`

**Steps:**

- [ ] **Step 1: Create module**

```rust
//! HTTP clients for the four supported embedding providers.
//!
//! Anthropic is intentionally absent — they don't expose an embedding model.

use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone)]
pub enum EmbeddingProvider {
    OpenAi { api_key: String },
    Gemini { api_key: String },
    Ollama { base_url: String },
}

impl EmbeddingProvider {
    pub fn from_id(
        provider_id: &str,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> TuskResult<Self> {
        match provider_id {
            "openai" => Ok(Self::OpenAi {
                api_key: api_key.ok_or_else(|| {
                    TuskError::AiNotConfigured("openai".into())
                })?,
            }),
            "gemini" => Ok(Self::Gemini {
                api_key: api_key.ok_or_else(|| {
                    TuskError::AiNotConfigured("gemini".into())
                })?,
            }),
            "ollama" => Ok(Self::Ollama {
                base_url: base_url
                    .unwrap_or_else(|| "http://localhost:11434".into()),
            }),
            "anthropic" => Err(TuskError::Ai(
                "Anthropic does not provide an embedding API".into(),
            )),
            other => Err(TuskError::Ai(format!("unknown provider: {other}"))),
        }
    }
}

pub async fn embed_one(
    client: &Client,
    provider: &EmbeddingProvider,
    model: &str,
    text: &str,
) -> TuskResult<Vec<f32>> {
    match provider {
        EmbeddingProvider::OpenAi { api_key } => embed_openai(client, api_key, model, text).await,
        EmbeddingProvider::Gemini { api_key } => embed_gemini(client, api_key, model, text).await,
        EmbeddingProvider::Ollama { base_url } => embed_ollama(client, base_url, model, text).await,
    }
}

async fn embed_openai(
    client: &Client,
    api_key: &str,
    model: &str,
    text: &str,
) -> TuskResult<Vec<f32>> {
    #[derive(Deserialize)]
    struct Resp {
        data: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        embedding: Vec<f32>,
    }
    let r = client
        .post("https://api.openai.com/v1/embeddings")
        .bearer_auth(api_key)
        .json(&json!({ "model": model, "input": text }))
        .send()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .error_for_status()
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .json::<Resp>()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?;
    r.data
        .into_iter()
        .next()
        .map(|i| i.embedding)
        .ok_or_else(|| TuskError::EmbeddingHttp("empty response".into()))
}

async fn embed_gemini(
    client: &Client,
    api_key: &str,
    model: &str,
    text: &str,
) -> TuskResult<Vec<f32>> {
    #[derive(Deserialize)]
    struct Resp {
        embedding: Inner,
    }
    #[derive(Deserialize)]
    struct Inner {
        values: Vec<f32>,
    }
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:embedContent?key={api_key}"
    );
    let r = client
        .post(&url)
        .json(&json!({
            "content": { "parts": [{ "text": text }] }
        }))
        .send()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .error_for_status()
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .json::<Resp>()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?;
    Ok(r.embedding.values)
}

async fn embed_ollama(
    client: &Client,
    base_url: &str,
    model: &str,
    text: &str,
) -> TuskResult<Vec<f32>> {
    #[derive(Deserialize)]
    struct Resp {
        embedding: Vec<f32>,
    }
    let url = format!(
        "{}/api/embeddings",
        base_url.trim_end_matches('/')
    );
    let r = client
        .post(&url)
        .json(&json!({ "model": model, "prompt": text }))
        .send()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .error_for_status()
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .json::<Resp>()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?;
    Ok(r.embedding)
}
```

- [ ] **Step 2: Register module**

In `src-tauri/src/db/mod.rs` add `pub mod embedding_http;`.

- [ ] **Step 3: Integration test against httpmock (no real network)**

Create `src-tauri/tests/embedding_http.rs`:

```rust
use httpmock::prelude::*;
use reqwest::Client;
use serde_json::json;

use tusk_lib::db::embedding_http::{embed_one, EmbeddingProvider};

#[tokio::test]
async fn ollama_roundtrip() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(POST).path("/api/embeddings");
        then.status(200).json_body(json!({ "embedding": [0.1, 0.2, 0.3] }));
    });
    let p = EmbeddingProvider::Ollama {
        base_url: server.base_url(),
    };
    let r = embed_one(&Client::new(), &p, "nomic-embed-text", "hello").await.unwrap();
    assert_eq!(r, vec![0.1, 0.2, 0.3]);
}

#[tokio::test]
async fn openai_roundtrip() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(POST).path("/v1/embeddings");
        then.status(200).json_body(json!({
            "data": [{ "embedding": [0.4, 0.5] }]
        }));
    });
    // The real client hardcodes api.openai.com — so this test exercises the
    // serde shape against a structurally-similar mock. We swap in by setting
    // the base URL via reqwest_middleware-style replacement is overkill here;
    // instead, expose `embed_openai_at` with an explicit endpoint param.
    // -> Refactor embed_openai to accept a base URL when this test fails.
    eprintln!("openai roundtrip is shape-only — refactor needed for true mocking");
}
```

> **Subagent note:** to make the OpenAI test airtight, refactor `embed_openai` to take an `endpoint: &str` parameter (default `https://api.openai.com/v1/embeddings`) so the test can point it at the mock. Same for Gemini. Treat this as a step in the same task — do not skip.

- [ ] **Step 4: Refactor for testability**

Add an internal helper `embed_openai_at(endpoint: &str, ...)` and have `embed_openai(...)` call it with the public URL. Same for Gemini. Update the test to construct the mock URL explicitly.

- [ ] **Step 5: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test embedding_http
```

Expected: green.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db/embedding_http.rs src-tauri/src/db/mod.rs src-tauri/tests/embedding_http.rs
git commit -m "feat(week4): embedding HTTP adapters (OpenAI / Gemini / Ollama) + httpmock test"
```

---

## Task 15: `commands/schema_index.rs` — sync_schema_index + progress events

**Goal:** Tauri command that walks all user tables in a connection, embeds each (skipping unchanged), upserts BLOBs, and emits progress events.

**Files:**
- Create: `src-tauri/src/commands/schema_index.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Steps:**

- [ ] **Step 1: Create command**

```rust
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::db::embedding_http::{embed_one, EmbeddingProvider};
use crate::db::embedding_store::{
    delete_for_conn, load_all, lookup_one, upsert_embedding,
};
use crate::db::pool::ConnectionRegistry;
use crate::db::schema_embed::{build_table_ddl, list_user_tables};
use crate::db::state::StateStore;
use crate::errors::{TuskError, TuskResult};
use crate::secrets;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub embedded: usize,
    pub skipped_unchanged: usize,
    pub failed: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    conn_id: String,
    state: &'static str,
    total_tables: usize,
    embedded_tables: usize,
    error_message: Option<String>,
}

#[tauri::command]
pub async fn sync_schema_index(
    app: AppHandle,
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
    embedding_provider: String,
    embedding_model: String,
    base_url: Option<String>,
) -> TuskResult<SyncReport> {
    let pool = registry.pool(&connection_id)?;
    let api_key = match embedding_provider.as_str() {
        "ollama" => None,
        other => secrets::ai_get(other)?,
    };
    let provider = EmbeddingProvider::from_id(
        &embedding_provider,
        api_key,
        base_url,
    )?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?;

    let tables = list_user_tables(&pool).await?;
    let total = tables.len();
    app.emit(
        "schema_index:progress",
        Progress {
            conn_id: connection_id.clone(),
            state: "running",
            total_tables: total,
            embedded_tables: 0,
            error_message: None,
        },
    )
    .ok();

    let mut report = SyncReport {
        embedded: 0,
        skipped_unchanged: 0,
        failed: Vec::new(),
    };
    for (i, (schema, table, oid)) in tables.into_iter().enumerate() {
        let ddl = match build_table_ddl(&pool, &schema, &table).await {
            Ok(d) => d,
            Err(e) => {
                report.failed.push(format!("{schema}.{table}: {e}"));
                continue;
            }
        };
        let needs_embed = match lookup_one(&store, &connection_id, &schema, &table)? {
            Some((relid, sum, model)) => {
                relid != oid || sum != ddl.checksum || model != embedding_model
            }
            None => true,
        };
        if !needs_embed {
            report.skipped_unchanged += 1;
        } else {
            match embed_one(&client, &provider, &embedding_model, &ddl.ddl).await {
                Ok(vec) => {
                    upsert_embedding(
                        &store,
                        &connection_id,
                        &schema,
                        &table,
                        oid,
                        &ddl.checksum,
                        &vec,
                        &embedding_model,
                        chrono::Utc::now().timestamp_millis(),
                    )?;
                    report.embedded += 1;
                }
                Err(e) => {
                    report.failed.push(format!("{schema}.{table}: {e}"));
                }
            }
        }
        if (i + 1) % 5 == 0 || i + 1 == total {
            app.emit(
                "schema_index:progress",
                Progress {
                    conn_id: connection_id.clone(),
                    state: "running",
                    total_tables: total,
                    embedded_tables: report.embedded + report.skipped_unchanged,
                    error_message: None,
                },
            )
            .ok();
        }
    }

    app.emit(
        "schema_index:done",
        Progress {
            conn_id: connection_id.clone(),
            state: "done",
            total_tables: total,
            embedded_tables: report.embedded + report.skipped_unchanged,
            error_message: None,
        },
    )
    .ok();

    Ok(report)
}

#[tauri::command]
pub fn schema_index_clear(
    store: State<'_, StateStore>,
    connection_id: String,
) -> TuskResult<()> {
    delete_for_conn(&store, &connection_id)
}

#[tauri::command]
pub fn schema_index_count(
    store: State<'_, StateStore>,
    connection_id: String,
) -> TuskResult<usize> {
    Ok(load_all(&store, &connection_id)?.len())
}
```

- [ ] **Step 2: Register module + tauri commands**

`src-tauri/src/commands/mod.rs`: add `pub mod schema_index;`.

`src-tauri/src/lib.rs` `invoke_handler!`:

```rust
            commands::schema_index::sync_schema_index,
            commands::schema_index::schema_index_clear,
            commands::schema_index::schema_index_count,
```

- [ ] **Step 3: Integration test (postgres + httpmock)**

Create `src-tauri/tests/schema_index.rs`:

```rust
use httpmock::prelude::*;
use serde_json::json;

#[tokio::test]
#[ignore] // requires postgres
async fn sync_against_small_schema() {
    // 1. bring up infra/postgres docker
    // 2. CREATE TABLE pets(id int primary key, name text);
    // 3. mock embedding server returns [0.1; 1536]
    // 4. invoke sync_schema_index
    // 5. assert SyncReport.embedded == 1, skipped == 0
    // 6. invoke again -> embedded == 0, skipped == 1
    let _server = MockServer::start();
    // (subagent: fill in once Week 3's pool / connection bootstrapping helper is wired in)
    eprintln!("placeholder — flesh out with test infra in T15 if helpers exist; otherwise leave for manual verification");
}
```

> **Subagent note:** if Week 3 added a test harness for connection bootstrapping (`tests/common/mod.rs` or similar), reuse it. Otherwise leave this as `#[ignore]`'d scaffold and rely on manual verification (T24).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/schema_index.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/tests/schema_index.rs
git commit -m "feat(week4): sync_schema_index command + progress events"
```

---

## Task 16: SchemaIndexPanel UI + auto-sync on connect

**Goal:** Settings → Schema Index tab shows progress for the active connection. Auto-sync fires on `connect()` success when `schemaIndexAutoSync` is true.

**Files:**
- Create: `src/store/schemaIndex.ts`
- Create: `src/features/settings/SchemaIndexPanel.tsx`
- Modify: `src/features/settings/SettingsDialog.tsx`
- Modify: `src/store/connections.ts` (post-connect hook)
- Modify: `src/App.tsx` (event listeners)

**Steps:**

- [ ] **Step 1: Progress mirror store**

Create `src/store/schemaIndex.ts`:

```ts
import { create } from "zustand";

import type { SchemaIndexProgress } from "@/lib/types";

interface SchemaIndexState {
  byConn: Record<string, SchemaIndexProgress>;
  set: (p: SchemaIndexProgress) => void;
  clear: (connId: string) => void;
}

export const useSchemaIndex = create<SchemaIndexState>((set) => ({
  byConn: {},
  set: (p) => set((s) => ({ byConn: { ...s.byConn, [p.connId]: p } })),
  clear: (connId) =>
    set((s) => {
      const next = { ...s.byConn };
      delete next[connId];
      return { byConn: next };
    }),
}));
```

- [ ] **Step 2: Listen for emits at app start**

In `src/App.tsx`, add an effect that subscribes to `schema_index:progress` and `schema_index:done`, mirroring payload into the store. Pattern matches the existing `query:started` / `query:completed` listeners — use the same cancel/cleanup shape.

- [ ] **Step 3: SchemaIndexPanel component**

Create `src/features/settings/SchemaIndexPanel.tsx` with: connection name, progress (running/done/idle), progress count, embedding provider/model, "Auto-sync on connect" checkbox, "Rebuild now" button, "Clear" button. Use `useSchemaIndex` for progress, `useConnections` for active conn, `useSettings` for auto-sync flag, `useAi` for embedding model/baseUrl. Buttons call `invoke('sync_schema_index', {...})` and `invoke('schema_index_clear', {...})`.

- [ ] **Step 4: Mount in SettingsDialog**

Replace the `schema-index` placeholder with `<SchemaIndexPanel />`.

- [ ] **Step 5: Auto-sync on connect**

In `src/store/connections.ts`, after a successful connect action populate `useSettings.getState().schemaIndexAutoSync` — when true, fire a fire-and-forget `invoke('sync_schema_index', { connectionId, embeddingProvider, embeddingModel, baseUrl })`. Catch + log on failure (no toast — silent in background).

- [ ] **Step 6: Verify**

```bash
pnpm typecheck && pnpm lint && pnpm format:check
```

Expected: PASS.

- [ ] **Step 7: Manual smoke**

```bash
pnpm tauri dev
```

Connect to a small Postgres → Settings → Schema Index → progress runs to done. Click Rebuild → second run mostly skipped.

- [ ] **Step 8: Commit**

```bash
git add src/store/schemaIndex.ts src/features/settings/SchemaIndexPanel.tsx src/features/settings/SettingsDialog.tsx src/store/connections.ts src/App.tsx
git commit -m "feat(week4): SchemaIndexPanel + auto-sync on connect"
```

---

## Task 17: schema_top_k + list_recent_queries commands

**Goal:** Two reads used at every Cmd+K invocation: (a) embed user prompt → return top-K relevant tables (with DDL + similarity + forced flag by name match); (b) last 5 successful queries on this connection.

**Files:**
- Modify: `src-tauri/src/commands/schema_index.rs`
- Modify: `src-tauri/src/commands/history.rs`
- Modify: `src-tauri/src/db/state.rs`
- Modify: `src-tauri/src/lib.rs`

**Steps:**

- [ ] **Step 1: Append `schema_top_k` to schema_index.rs**

Add types `TopKTable { schema, table, ddl, similarity, forced }` and `SchemaTopK { tables, totalTables }`. Add `#[tauri::command] async fn schema_top_k(...)` that:

1. Acquires the registry pool for `connection_id`.
2. Loads all stored embeddings via `embedding_store::load_all`.
3. Embeds `user_prompt` via `embedding_http::embed_one`.
4. Runs `cosine_top_k(query, rows, top_k)`.
5. Tokenizes `user_prompt` (split on non-alphanumeric/underscore, lowercase) and forces inclusion of any row whose `table` or `schema.table` matches a token. Sets `forced = true`.
6. For each chosen row, calls `build_table_ddl` to produce fresh DDL (tolerating drift).
7. Returns `SchemaTopK { tables, total_tables }`.

- [ ] **Step 2: Add `list_recent_successful` to history.rs**

Add a new tauri command that wraps a sqlite read of `history_entry` filtered by `conn_id` and `status = 'ok'`, ordered by `started_at DESC LIMIT ?`. Reuse Week 3's existing row mapper for `HistoryEntry` rather than rewriting it.

- [ ] **Step 3: Register commands**

In `src-tauri/src/lib.rs` `invoke_handler!`:

```
commands::schema_index::schema_top_k,
commands::history::list_recent_successful,
```

- [ ] **Step 4: Verify**

```bash
pnpm rust:check && pnpm rust:lint
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/schema_index.rs src-tauri/src/commands/history.rs src-tauri/src/db/state.rs src-tauri/src/lib.rs
git commit -m "feat(week4): schema_top_k + list_recent_successful"
```

---

## Task 18: Frontend prompts + tools modules

**Goal:** Pure functions — `buildSystemPrompt(args)` synthesizes the system prompt (PG version + extensions + top-K DDL + recent queries + safety rules). `buildTools(deps)` returns the AI SDK tool object.

**Files:**
- Create: `src/lib/ai/types.ts`
- Create: `src/lib/ai/prompts.ts`
- Create: `src/lib/ai/prompts.test.ts`
- Create: `src/lib/ai/tools.ts`

**Steps:**

- [ ] **Step 1: Local types** (`src/lib/ai/types.ts`)

Define `TopKTable` and `SchemaTopK` matching the Rust serialization shape (camelCase). Re-export from `@/lib/ai/types`.

- [ ] **Step 2: prompts.ts**

`buildSystemPrompt(args)` returns a string composed of:

1. Header: `You are an expert PostgreSQL author. Target Postgres <version>.`
2. Active extensions line.
3. Output rules: single ```sql fence, no other prose, schema-qualified names, do NOT invent identifiers, destructive operations require user confirmation, brief `--` comment when assumption needed.
4. `Schema (top-K):` block — for each `TopKTable`, header `-- schema.table (sim=X.XX)` or `(forced include)`, followed by raw DDL.
5. If recent queries: `Recent successful queries on this connection:` block with each prefixed by `-- recent`.
6. If `selectionContext` present: instruction to edit existing SQL block.

Also export `extractSql(text)` — extracts the first ```sql fenced block, or returns trimmed text if no fence.

- [ ] **Step 3: tools.ts**

`buildTools({ connectionId, sampleRowsEnabled })` returns an object whose keys are AI-SDK `tool()` definitions. Each tool's executor delegates to the Tauri commands from T19 (`get_table_schema` / `list_indexes` / `sample_rows`). Use `zod` for input schemas. Spread `sample_rows` only when `sampleRowsEnabled` is true.

> **Subagent note:** Vercel AI SDK 6's tool helper exposes `inputSchema` (some 5.x versions used `parameters`). Look at `node_modules/ai/dist/index.d.ts` to choose the correct field name.

- [ ] **Step 4: prompts.test.ts**

Vitest cases:
- `buildSystemPrompt` includes pg version, extension names, schema-qualified DDL, recent queries.
- `buildSystemPrompt` with `selectionContext` includes the selection block.
- `extractSql` returns content of ```sql fenced block.
- `extractSql` returns trimmed input when no fence present.

- [ ] **Step 5: Run vitest**

```bash
pnpm test
```

Expected: green.

- [ ] **Step 6: Commit**

```bash
git add src/lib/ai/prompts.ts src/lib/ai/prompts.test.ts src/lib/ai/tools.ts src/lib/ai/types.ts
git commit -m "feat(week4): system prompt builder + AI SDK tool defs"
```

---

## Task 19: Rust ai_tools commands (`get_table_schema`, `list_indexes`, `sample_rows`)

**Goal:** Backend implementations of the tool calls invoked by the LLM via AI SDK. `sample_rows` returns query-result-shaped data and is gated at the frontend (T18 `buildTools` filter).

**Files:**
- Create: `src-tauri/src/commands/ai_tools.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/tests/ai_tools.rs`

**Steps:**

- [ ] **Step 1: get_table_schema**

Tauri command that takes `connection_id, schema, table`, acquires the pool, calls `build_table_ddl`, returns `String`.

- [ ] **Step 2: list_indexes**

Tauri command that runs:

```sql
SELECT i.relname, pg_get_indexdef(ix.indexrelid),
       ix.indisunique, ix.indisprimary
FROM pg_index ix
JOIN pg_class i ON i.oid = ix.indexrelid
WHERE ix.indrelid = ($1 || '.' || $2)::regclass
ORDER BY i.relname
```

Returns `Vec<IndexRow { name, definition, is_unique, is_primary }>` with camelCase wire format.

- [ ] **Step 3: sample_rows**

Tauri command that builds `SELECT * FROM "schema"."table" LIMIT N` (clamping `N` to 20). Reuse Week 3's `db::decoder::{columns_of, decode_row, Cell}` (the actual API may differ — read decoder.rs first, follow exactly). Return `serde_json::Value` with `{ columns, rows }`.

- [ ] **Step 4: Register**

`commands/mod.rs` add `pub mod ai_tools;`. `lib.rs` register all three commands.

- [ ] **Step 5: Verify**

```bash
pnpm rust:check && pnpm rust:lint
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 6: Integration test scaffold**

Create `src-tauri/tests/ai_tools.rs` with `#[ignore]`'d tests for each command. Manual verification covers them.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/ai_tools.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/tests/ai_tools.rs
git commit -m "feat(week4): ai_tools — get_table_schema / list_indexes / sample_rows"
```

---

## Task 20: CmdKPalette component + streaming wrapper

**Goal:** Floating modal anchored near the editor cursor. Takes user prompt → assembles system prompt via T18 → calls AI SDK `streamText` → displays SQL as it arrives → emits a final `generated_sql` to the parent. Supports AbortController (esc/re-prompt cancels in-flight stream).

**Files:**
- Create: `src/lib/ai/stream.ts`
- Create: `src/features/ai/CmdKPalette.tsx`

**Steps:**

- [ ] **Step 1: stream.ts wrapper**

```ts
import { streamText, type LanguageModel } from "ai";

import { buildTools } from "@/lib/ai/tools";

export interface StreamGenerationArgs {
  model: LanguageModel;
  systemPrompt: string;
  userPrompt: string;
  connectionId: string;
  sampleRowsEnabled: boolean;
  signal: AbortSignal;
  onChunk: (text: string) => void;
}

export interface StreamGenerationResult {
  text: string;
  toolCalls: { name: string; args: unknown }[];
  promptTokens?: number;
  completionTokens?: number;
}

export async function streamGeneration(
  args: StreamGenerationArgs,
): Promise<StreamGenerationResult> {
  const tools = buildTools({
    connectionId: args.connectionId,
    sampleRowsEnabled: args.sampleRowsEnabled,
  });
  const result = streamText({
    model: args.model,
    system: args.systemPrompt,
    prompt: args.userPrompt,
    tools,
    abortSignal: args.signal,
    maxRetries: 1,
  });

  let buf = "";
  for await (const delta of result.textStream) {
    buf += delta;
    args.onChunk(buf);
  }
  const finalCalls: { name: string; args: unknown }[] = [];
  try {
    const calls = await result.toolCalls;
    for (const c of calls) {
      finalCalls.push({ name: c.toolName, args: c.input });
    }
  } catch {
    // some providers stream-only; ignore
  }
  const usage = await result.usage.catch(() => undefined);
  return {
    text: buf,
    toolCalls: finalCalls,
    promptTokens: usage?.promptTokens,
    completionTokens: usage?.completionTokens,
  };
}
```

> **Subagent note:** AI SDK 6 result property names may differ (`textStream` / `toolCalls` / `usage`). Inspect `node_modules/ai/dist/index.d.ts` for the actual surface; adapt names while keeping the public function signature.

- [ ] **Step 2: CmdKPalette component**

```tsx
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { extractSql } from "@/lib/ai/prompts";
import { buildSystemPrompt } from "@/lib/ai/prompts";
import { buildModel } from "@/lib/ai/providers";
import { streamGeneration } from "@/lib/ai/stream";
import { aiSecretGet } from "@/lib/keychain";
import type { SchemaTopK } from "@/lib/ai/types";
import { useAi } from "@/store/ai";
import { useSettings } from "@/store/settings";

interface Props {
  open: boolean;
  connectionId: string | undefined;
  selection: string;
  onClose: () => void;
  onApply: (sql: string, meta: ApplyMeta) => void;
}

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
      const pgVersion = "16"; // TODO: fetch via dedicated command in v1.5
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
    <div
      className="fixed inset-0 z-40"
      onClick={onClose}
      role="presentation"
    >
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
          <Button size="sm" disabled={busy || prompt.trim().length === 0} onClick={onSubmit}>
            {busy ? "Streaming…" : "Generate"}
          </Button>
        </div>
        <p className="text-muted-foreground mt-1 text-xs">
          {defaultGen} · {cfg.generationModel} · top-K {ragTopK}
        </p>
        {streamed && (
          <pre className="bg-muted mt-3 max-h-64 overflow-auto rounded p-2 text-xs">
            {streamed}
          </pre>
        )}
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
```

- [ ] **Step 3: Verify**

```bash
pnpm typecheck && pnpm lint
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/lib/ai/stream.ts src/features/ai/CmdKPalette.tsx
git commit -m "feat(week4): CmdKPalette + streaming generation wrapper"
```

---

## Task 21: SqlDiffView (Monaco DiffEditor)

**Goal:** When the user has a selection in editor, show side-by-side diff (original / generated). When no selection, just show the generated SQL pretty-printed. CmdKPalette uses this for the preview area.

**Files:**
- Create: `src/features/ai/SqlDiffView.tsx`
- Modify: `src/features/ai/CmdKPalette.tsx` (replace `<pre>` with diff when selection present)

**Steps:**

- [ ] **Step 1: Create component**

```tsx
import { DiffEditor } from "@monaco-editor/react";

import { useTheme } from "@/hooks/use-theme";

interface Props {
  original: string;
  modified: string;
  height?: number;
}

export function SqlDiffView({ original, modified, height = 240 }: Props) {
  const { theme } = useTheme();
  return (
    <DiffEditor
      original={original}
      modified={modified}
      language="sql"
      theme={theme === "dark" ? "vs-dark" : "vs"}
      height={height}
      options={{
        renderSideBySide: true,
        readOnly: true,
        minimap: { enabled: false },
        scrollBeyondLastLine: false,
        fontSize: 12,
      }}
    />
  );
}
```

- [ ] **Step 2: Use it from CmdKPalette**

In `CmdKPalette.tsx`, replace the `streamed && <pre>...</pre>` block with:

```tsx
{streamed && (
  selection ? (
    <div className="mt-3">
      <SqlDiffView original={selection} modified={streamed} />
    </div>
  ) : (
    <pre className="bg-muted mt-3 max-h-64 overflow-auto rounded p-2 text-xs">
      {streamed}
    </pre>
  )
)}
```

Add `import { SqlDiffView } from "./SqlDiffView";`.

- [ ] **Step 3: Verify + manual smoke**

```bash
pnpm typecheck && pnpm lint && pnpm tauri dev
```

Open Cmd+K (still wired in T22) without selection → plain block. With selection → side-by-side diff.

- [ ] **Step 4: Commit**

```bash
git add src/features/ai/SqlDiffView.tsx src/features/ai/CmdKPalette.tsx
git commit -m "feat(week4): SqlDiffView (Monaco diff) for selection-mode generation"
```

---

## Task 22: EditorPane Cmd+K integration + Apply/Reject + AI history record

**Goal:** Cmd+K shortcut in the editor mounts the palette. Apply replaces the selection (if any) or inserts at cursor; record an `ai_history` row + `history_entry` row.

**Files:**
- Modify: `src/features/editor/EditorPane.tsx`
- Modify: `src-tauri/src/commands/history.rs` (add `record_ai_generation`)
- Modify: `src-tauri/src/db/state.rs` (insert into `history_entry` + `ai_history`)
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/features/history/HistoryPalette.tsx` (render `source='ai'` rows with ✦ icon)

**Steps:**

- [ ] **Step 1: Backend `record_ai_generation` command**

In `src-tauri/src/commands/history.rs` add a tauri command:

```rust
#[tauri::command]
pub fn record_ai_generation(
    store: State<'_, StateStore>,
    payload: AiGenerationPayload,
) -> TuskResult<String> {
    crate::db::state::insert_ai_generation(&store, payload)
        .map_err(|e| TuskError::History(e.to_string()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGenerationPayload {
    pub conn_id: String,
    pub prompt: String,
    pub generated_sql: String,
    pub provider: String,
    pub generation_model: String,
    pub embedding_model: Option<String>,
    pub top_k_tables: Vec<String>,
    pub tool_calls: serde_json::Value,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub duration_ms: i64,
}
```

In `state.rs` add a single-transaction insert that:

1. Generates a UUID `entry_id`.
2. Inserts into `history_entry`: `source='ai'`, `sql_preview = first 200 of generated_sql`, `sql_full = generated_sql`, `started_at = now`, `duration_ms`, `status='ok'`, `statement_count = 1`.
3. Inserts into `ai_history` with the rest of the metadata (top_k_tables / tool_calls serialized as JSON).
4. Returns the entry id.

Mind the existing `source` enum check — if `state.rs` uses a Rust-side validator, extend it to accept `'ai'`.

- [ ] **Step 2: Register command**

`src-tauri/src/lib.rs`:

```
commands::history::record_ai_generation,
```

- [ ] **Step 3: Cmd+K shortcut in EditorPane**

In `src/features/editor/EditorPane.tsx`:

- Import `CmdKPalette` and a small ref accessor that returns the current selection text + cursor offset.
- Add state `const [showCmdK, setShowCmdK] = useState(false);` and `const [selection, setSelection] = useState("");`.
- In the existing `useEffect(... onKey ...)`, add another branch:

```ts
if (e.key.toLowerCase() === "k") {
  e.preventDefault();
  const ed = editorRef.current;
  let sel = "";
  if (ed) {
    const m = ed.getModel();
    const r = ed.getSelection();
    if (m && r) sel = m.getValueInRange(r);
  }
  setSelection(sel);
  setShowCmdK(true);
}
```

- Render below the diff editor:

```tsx
<CmdKPalette
  open={showCmdK}
  connectionId={connectionForTab ?? undefined}
  selection={selection}
  onClose={() => setShowCmdK(false)}
  onApply={async (sql, meta) => {
    const ed = editorRef.current;
    if (ed) {
      const m = ed.getModel();
      const r = ed.getSelection();
      if (m && r) {
        // Replace selection (or insert at cursor when r is empty)
        ed.executeEdits("cmdk-apply", [{ range: r, text: sql, forceMoveMarkers: true }]);
      } else {
        // Fallback: append
        updateSql(activeTab.id, activeTab.sql + (activeTab.sql.endsWith("\n") ? "" : "\n") + sql);
      }
    }
    if (connectionForTab) {
      try {
        await invoke("record_ai_generation", {
          payload: {
            connId: connectionForTab,
            prompt: meta.prompt,
            generatedSql: sql,
            provider: meta.provider,
            generationModel: meta.generationModel,
            embeddingModel: meta.embeddingModel,
            topKTables: meta.topKTables,
            toolCalls: meta.toolCalls,
            promptTokens: meta.promptTokens ?? null,
            completionTokens: meta.completionTokens ?? null,
            durationMs: 0,
          },
        });
      } catch (e) {
        toast.error(`Failed to record AI history: ${e}`);
      }
    }
    setShowCmdK(false);
  }}
/>
```

Add `import { invoke } from "@tauri-apps/api/core";` and the `import { CmdKPalette } from "@/features/ai/CmdKPalette";` at the top.

- [ ] **Step 4: HistoryPalette `source='ai'` rendering**

In `src/features/history/HistoryPalette.tsx`, when rendering an entry, if `entry.source === 'ai'` prepend a small `✦` icon and render the entry preview as `AI: <sql_preview>`. Re-use any existing `HistoryEntry` types — extend the union to include `'ai'` if it's currently restricted.

- [ ] **Step 5: Verify**

```bash
pnpm typecheck && pnpm lint && pnpm format:check
pnpm rust:check && pnpm rust:lint && cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 6: Manual smoke**

Connect to local Postgres → ensure schema synced → press `Cmd+K` in editor with no selection → ask "list 10 most recent users" → wait for stream → Apply → SQL appears in editor → Cmd+Enter → result grid populates.

Repeat with selection: select existing SQL → Cmd+K → "add a join to orders table" → diff view shows original vs new → Apply → selection replaced.

Open Cmd+P palette → AI entry shows with ✦ icon and prompt preview.

- [ ] **Step 7: Commit**

```bash
git add src/features/editor/EditorPane.tsx src/features/history/HistoryPalette.tsx src-tauri/src/commands/history.rs src-tauri/src/db/state.rs src-tauri/src/lib.rs
git commit -m "feat(week4): Cmd+K integration + AI history record"
```

---

## Task 23: Final test pass + Week 2/3 regression

**Goal:** All gates green; vitest covers Week 4 critical units; integration tests run via docker postgres; no regressions in Week 2/3 functionality.

**Files:** none new

**Steps:**

- [ ] **Step 1: Frontend gates**

```bash
pnpm typecheck
pnpm lint
pnpm format:check
pnpm test
pnpm build
```

All green.

- [ ] **Step 2: Rust gates**

```bash
pnpm rust:check
pnpm rust:lint
pnpm rust:fmt:check
cargo test --manifest-path src-tauri/Cargo.toml
docker compose -f infra/postgres/docker-compose.yml up -d
cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored
```

All green.

- [ ] **Step 3: Re-run Week 2/3 manual verification quickly**

```bash
cat docs/superpowers/plans/manual-verification-week-2.md
cat docs/superpowers/plans/manual-verification-week-3.md
```

Spot-check the most likely-to-break items: connection add, schema tree, Cmd+Enter execute, inline edit submit (single row), tx_begin/commit, Cmd+P palette, CSV export. None should regress.

- [ ] **Step 4: Commit (no-op if clean)**

If any test stabilization landed:

```bash
git commit -am "test(week4): stabilize edge cases discovered in final pass"
```

Otherwise skip.

---

## Task 24: Manual verification document

**Goal:** A reproducible checklist a non-author can run end-to-end.

**Files:**
- Create: `docs/superpowers/plans/manual-verification-week-4.md`

**Steps:**

- [ ] **Step 1: Write checklist**

Create the document with the following sections (concise, checkbox per item):

1. **Setup**
   - `pnpm install`, `pnpm tauri dev`.
   - Docker Postgres up, seed at least 5 tables (e.g., `users`, `orders`, `products`, `payments`, `audit_log`).

2. **BYOK**
   - For each provider (OpenAI / Anthropic / Gemini / Ollama):
     - Open Settings → Providers → enter key → Save → "key set" badge appears.
     - Test → toast "pong" or generic ack within 5s.
     - Remove → "no key" badge.

3. **Schema index**
   - Connect to seeded Postgres → Schema Index panel shows progress → done with N embedded.
   - Click Rebuild → second run mostly skipped (skipped_unchanged increments, embedded ≈ 0).
   - `ALTER TABLE users ADD COLUMN nickname text` via editor → Cmd+K next time → users DDL re-embedded (verify via Schema Index panel + Postgres state).

4. **Cmd+K — generate**
   - No selection → "list users from last week" → SQL stream appears → Apply → editor populated.
   - With selection → "filter to paid customers" → diff view side-by-side → Apply → selection replaced.
   - Reject → editor unchanged.
   - Re-prompt → previous stream cancels.

5. **Cmd+K — tools**
   - Prompt: "show columns of orders" → expected: model invokes `get_table_schema` (visible in network/DevTools? or via ai_history.tool_calls in `tusk.db`).
   - Toggle `sample_rows` ON → prompt with "what does the data look like in users?" → tool call recorded.

6. **Destructive guard — typed SQL**
   - Type `DROP TABLE foo;` → Cmd+Enter → modal → Cancel = no execution. Run anyway = execution attempt (DB error if absent — expected).
   - Type `DELETE FROM users;` → modal → "DELETE without WHERE will remove all rows from public.users".
   - `DELETE FROM users WHERE id = 1;` → no modal.
   - Strict mode ON → modal requires typing `DELETE` → button enables on exact match.

7. **Destructive guard — AI generated**
   - Cmd+K → "remove all rows from audit_log" → generated DELETE/TRUNCATE → Apply → Cmd+Enter → modal triggers as for typed SQL.

8. **Errors**
   - Wrong API key → Test fails with provider message.
   - Disconnect network → Cmd+K → toast error within timeout.
   - Aborted stream → toast "Generation cancelled".
   - Embedding provider not set → Cmd+K → toast "Embedding model not set".

9. **History**
   - Cmd+P palette → AI entries show ✦ + "AI: <prompt preview>".
   - Click → loads `generated_sql` into editor.

10. **Regression**
    - All Week 2/3 manual verification items still pass.

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/plans/manual-verification-week-4.md
git commit -m "docs(week4): manual verification checklist"
```

---

## Closing checklist

- [ ] All 24 tasks complete with green gates.
- [ ] Week 4 manual verification doc round-tripped at least once.
- [ ] No `TODO` / `FIXME` left in shipped code (only in scaffolds explicitly marked as such).
- [ ] No raw API keys land in `localStorage`, zustand persist payloads, or git.
- [ ] All commits use the convention from the header (no AI / Co-Authored-By trailers).
- [ ] Final tag/PR opens against `main`.

**Done.**

