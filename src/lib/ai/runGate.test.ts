import { afterEach, describe, expect, it, vi, type Mock } from "vitest";

vi.mock("@/lib/ai/destructive", () => ({
  classifyDestructive: vi.fn(),
}));
vi.mock("@/features/ai/DestructiveModal", () => ({
  confirmDestructive: vi.fn(),
}));
vi.mock("@/store/settings", () => ({
  useSettings: { getState: () => ({ destructiveStrict: false }) },
}));

import { runGate } from "./runGate";
import { classifyDestructive } from "@/lib/ai/destructive";
import { confirmDestructive } from "@/features/ai/DestructiveModal";

afterEach(() => vi.clearAllMocks());

describe("runGate", () => {
  it("returns true with no findings", async () => {
    (classifyDestructive as unknown as Mock).mockResolvedValue([]);
    expect(await runGate("SELECT 1")).toBe(true);
    expect(confirmDestructive).not.toHaveBeenCalled();
  });

  it("delegates to confirmDestructive when findings exist", async () => {
    (classifyDestructive as unknown as Mock).mockResolvedValue([
      { kind: "drop-table", statementIndex: 0, message: "x" },
    ]);
    (confirmDestructive as unknown as Mock).mockResolvedValue(true);
    expect(await runGate("DROP TABLE x")).toBe(true);
    expect(confirmDestructive).toHaveBeenCalledOnce();
  });

  it("returns false on cancel", async () => {
    (classifyDestructive as unknown as Mock).mockResolvedValue([
      { kind: "truncate", statementIndex: 0, message: "x" },
    ]);
    (confirmDestructive as unknown as Mock).mockResolvedValue(false);
    expect(await runGate("TRUNCATE x")).toBe(false);
  });
});
