import { describe, it, expect, beforeEach } from "vitest";
import { usePendingChanges } from "./pendingChanges";
import type { Cell } from "@/lib/types";

const T = { schema: "public", name: "users" };

describe("pendingChanges store", () => {
  beforeEach(() => {
    usePendingChanges.getState().revertAll();
  });

  it("upsert creates a row entry on first edit", () => {
    usePendingChanges.getState().upsertEdit({
      table: T,
      pkColumns: ["id"],
      pkValues: [{ kind: "Int", value: 1 } satisfies Cell],
      column: "name",
      original: { kind: "Text", value: "old" },
      next: { kind: "Text", value: "new" },
      capturedRow: [
        { kind: "Int", value: 1 },
        { kind: "Text", value: "old" },
      ],
      capturedColumns: ["id", "name"],
    });
    expect(usePendingChanges.getState().count()).toBe(1);
  });

  it("upsert on the same column overwrites", () => {
    const args = (next: string) => ({
      table: T,
      pkColumns: ["id"],
      pkValues: [{ kind: "Int", value: 1 } as Cell],
      column: "name",
      original: { kind: "Text", value: "old" } as Cell,
      next: { kind: "Text", value: next } as Cell,
      capturedRow: [
        { kind: "Int", value: 1 } as Cell,
        { kind: "Text", value: "old" } as Cell,
      ],
      capturedColumns: ["id", "name"],
    });
    usePendingChanges.getState().upsertEdit(args("a"));
    usePendingChanges.getState().upsertEdit(args("b"));
    const list = usePendingChanges.getState().list();
    expect(list).toHaveLength(1);
    expect(list[0].edits).toHaveLength(1);
    if (list[0].edits[0].next.kind === "Text") {
      expect(list[0].edits[0].next.value).toBe("b");
    }
  });

  it("revertRow drops the entry", () => {
    usePendingChanges.getState().upsertEdit({
      table: T,
      pkColumns: ["id"],
      pkValues: [{ kind: "Int", value: 1 } as Cell],
      column: "name",
      original: { kind: "Text", value: "a" },
      next: { kind: "Text", value: "b" },
      capturedRow: [
        { kind: "Int", value: 1 } as Cell,
        { kind: "Text", value: "a" } as Cell,
      ],
      capturedColumns: ["id", "name"],
    });
    const key = JSON.stringify([{ kind: "Int", value: 1 }]);
    usePendingChanges.getState().revertRow(key);
    expect(usePendingChanges.getState().count()).toBe(0);
  });
});
