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
  {
    kind: "update-no-where",
    re: /\bupdate\s+\S+\s+set\b(?![\s\S]*\bwhere\b)/i,
  },
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
