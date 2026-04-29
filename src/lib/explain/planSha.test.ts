import { describe, expect, it } from "vitest";
import { planSha, stableStringify } from "./planSha";

describe("stableStringify", () => {
  it("produces identical output for objects with different key order", () => {
    expect(stableStringify({ a: 1, b: 2 })).toBe(
      stableStringify({ b: 2, a: 1 }),
    );
  });

  it("preserves array order", () => {
    expect(stableStringify([1, 2])).not.toBe(stableStringify([2, 1]));
  });

  it("recurses into nested objects", () => {
    const a = { x: { c: 1, b: 2 }, y: [{ z: 1, a: 2 }] };
    const b = { y: [{ a: 2, z: 1 }], x: { b: 2, c: 1 } };
    expect(stableStringify(a)).toBe(stableStringify(b));
  });
});

describe("planSha", () => {
  it("produces a 64-char hex string", async () => {
    const sha = await planSha({ Plan: { "Node Type": "Seq Scan" } });
    expect(sha).toMatch(/^[0-9a-f]{64}$/);
  });

  it("is stable across key re-orderings", async () => {
    const a = await planSha({
      Plan: { "Node Type": "Seq Scan", "Total Cost": 1 },
    });
    const b = await planSha({
      Plan: { "Total Cost": 1, "Node Type": "Seq Scan" },
    });
    expect(a).toBe(b);
  });
});
