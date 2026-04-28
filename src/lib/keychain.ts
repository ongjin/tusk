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
