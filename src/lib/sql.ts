const LIMIT_RE = /\blimit\s+\d+\b/i;
const SELECT_RE = /^\s*select\b/i;

export function withAutoLimit(sql: string, limit = 1000): string {
  if (!SELECT_RE.test(sql)) return sql;
  if (LIMIT_RE.test(sql)) return sql;
  // strip trailing semicolon for the merge
  const trimmed = sql.replace(/;\s*$/, "");
  return `${trimmed} LIMIT ${limit}`;
}
