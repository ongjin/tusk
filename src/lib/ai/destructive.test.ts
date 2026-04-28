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
