import type { Cell } from "@/lib/types";

export function cellAsString(cell: Cell): string {
  switch (cell.kind) {
    case "Null":
      return "";
    case "Bool":
      return cell.value ? "true" : "false";
    case "Int":
    case "Float":
      return String(cell.value);
    case "Bigint":
    case "Numeric":
    case "Text":
    case "Uuid":
    case "Inet":
    case "Date":
    case "Time":
    case "Timetz":
    case "Timestamp":
    case "Timestamptz":
      return cell.value;
    case "Interval":
      return cell.value.iso;
    case "Bytea":
      return cell.value.b64;
    case "Json":
      return JSON.stringify(cell.value);
    case "Array":
      return JSON.stringify(cell.value);
    case "Enum":
      return cell.value.value;
    case "Vector":
      return JSON.stringify(cell.value.values);
    case "Unknown":
      return cell.value.text;
  }
}

// Text-only fallback parser. Real type-aware widgets land in Tasks 12-15.
// Preserves the kind of `raw`; treats empty string as Null.
export function parseCellLike(text: string, raw: Cell): Cell {
  const trimmed = text.trim();
  if (trimmed === "") {
    return { kind: "Null" };
  }
  switch (raw.kind) {
    case "Null":
      // raw was null but user typed something — fall back to Text.
      return { kind: "Text", value: text };
    case "Bool": {
      const lc = trimmed.toLowerCase();
      if (lc === "true" || lc === "t" || lc === "1") {
        return { kind: "Bool", value: true };
      }
      if (lc === "false" || lc === "f" || lc === "0") {
        return { kind: "Bool", value: false };
      }
      return { kind: "Bool", value: raw.value };
    }
    case "Int": {
      const n = Number(trimmed);
      return Number.isFinite(n) && Number.isInteger(n)
        ? { kind: "Int", value: n }
        : raw;
    }
    case "Float": {
      const n = Number(trimmed);
      return Number.isFinite(n) ? { kind: "Float", value: n } : raw;
    }
    case "Bigint":
      return { kind: "Bigint", value: trimmed };
    case "Numeric":
      return { kind: "Numeric", value: trimmed };
    case "Text":
      return { kind: "Text", value: text };
    case "Uuid":
      return { kind: "Uuid", value: trimmed };
    case "Inet":
      return { kind: "Inet", value: trimmed };
    case "Date":
      return { kind: "Date", value: trimmed };
    case "Time":
      return { kind: "Time", value: trimmed };
    case "Timetz":
      return { kind: "Timetz", value: trimmed };
    case "Timestamp":
      return { kind: "Timestamp", value: trimmed };
    case "Timestamptz":
      return { kind: "Timestamptz", value: trimmed };
    case "Interval":
      return { kind: "Interval", value: { iso: trimmed } };
    case "Bytea":
      return { kind: "Bytea", value: { b64: trimmed } };
    case "Json": {
      try {
        return { kind: "Json", value: JSON.parse(text) };
      } catch {
        return raw;
      }
    }
    case "Array":
      return raw; // edited via real widget in later task
    case "Enum":
      return {
        kind: "Enum",
        value: { typeName: raw.value.typeName, value: trimmed },
      };
    case "Vector": {
      try {
        const parsed = JSON.parse(text);
        if (
          Array.isArray(parsed) &&
          parsed.every((n) => typeof n === "number")
        ) {
          return {
            kind: "Vector",
            value: { dim: parsed.length, values: parsed as number[] },
          };
        }
        return raw;
      } catch {
        return raw;
      }
    }
    case "Unknown":
      return {
        kind: "Unknown",
        value: { oid: raw.value.oid, text },
      };
  }
}

export function cellsEqual(a: Cell, b: Cell): boolean {
  if (a.kind !== b.kind) return false;
  return JSON.stringify(a) === JSON.stringify(b);
}
