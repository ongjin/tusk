import { describe, expect, it } from "vitest";

import { formatVectorSummary, l2Norm } from "./cellRender";

describe("l2Norm", () => {
  it("computes for [3,4]", () => {
    expect(l2Norm([3, 4])).toBeCloseTo(5);
  });
  it("zero vector → 0", () => {
    expect(l2Norm([0, 0, 0])).toBe(0);
  });
  it("empty → 0", () => {
    expect(l2Norm([])).toBe(0);
  });
});

describe("formatVectorSummary", () => {
  it("includes dim + norm", () => {
    expect(formatVectorSummary([3, 4])).toBe("[2d, ‖v‖=5.000]");
  });
});
