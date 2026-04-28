// src/lib/pgLiterals.ts
//
// TS mirror of Rust's `db::pg_literals::to_literal`. Used by client-side
// preview / "Copy as INSERT" to render typed cells as PG literal SQL
// fragments without a Tauri round-trip. Parity with the Rust impl is
// covered by a vitest spec in Task 23.
//
// Trust model: assumes string-payload variants come from the typed decoder.
// Do not feed user-supplied SQL fragments here.

import type { Cell } from "./types";

export function toLiteral(c: Cell): string {
  switch (c.kind) {
    case "Null":
      return "NULL";
    case "Bool":
      return c.value ? "TRUE" : "FALSE";
    case "Int":
      return String(c.value);
    case "Float":
      return formatFloat(c.value);
    case "Bigint":
    case "Numeric":
      return c.value;
    case "Text":
      return quote(c.value);
    case "Bytea":
      return `'\\x${b64ToHex(c.value.b64)}'::bytea`;
    case "Uuid":
      return `${quote(c.value)}::uuid`;
    case "Inet":
      return `${quote(c.value)}::inet`;
    case "Date":
      return `${quote(c.value)}::date`;
    case "Time":
      return `${quote(c.value)}::time`;
    case "Timetz":
      return `${quote(c.value)}::timetz`;
    case "Timestamp":
      return `${quote(c.value)}::timestamp`;
    case "Timestamptz":
      return `${quote(c.value)}::timestamptz`;
    case "Interval":
      return `${quote(c.value.iso)}::interval`;
    case "Json":
      return `${quote(JSON.stringify(c.value))}::jsonb`;
    case "Enum":
      return `${quote(c.value.value)}::${c.value.typeName}`;
    case "Array": {
      const inner = c.value.values.map(toLiteral).join(",");
      return `ARRAY[${inner}]::${c.value.elem}[]`;
    }
    case "Vector": {
      const parts = c.value.values.map((n) =>
        Number.isFinite(n) ? formatFloat(n) : "0",
      );
      return `${quote(`[${parts.join(",")}]`)}::vector`;
    }
    case "Unknown":
      return `${quote(c.value.text)}::text`;
  }
}

function quote(s: string): string {
  return `'${s.replace(/'/g, "''")}'`;
}

function b64ToHex(b64: string): string {
  const bin = atob(b64);
  let out = "";
  for (let i = 0; i < bin.length; i++) {
    out += bin.charCodeAt(i).toString(16).padStart(2, "0");
  }
  return out;
}

// Mirrors Rust `format_float`: NaN/Infinity → quoted PG forms, integral
// values within ±1e16 → no decimal, else default JS string repr.
function formatFloat(v: number): string {
  if (Number.isNaN(v)) return "'NaN'::float8";
  if (!Number.isFinite(v)) {
    return v < 0 ? "'-Infinity'::float8" : "'Infinity'::float8";
  }
  if (Number.isInteger(v) && Math.abs(v) < 1e16) {
    return String(v);
  }
  return String(v);
}
