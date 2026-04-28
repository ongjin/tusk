import { afterEach, describe, expect, it } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";

import { DestructiveModalHost, confirmDestructive } from "./DestructiveModal";

afterEach(cleanup);

describe("DestructiveModal", () => {
  it("standard mode: clicking Run anyway resolves true", async () => {
    render(<DestructiveModalHost />);
    const promise = confirmDestructive({
      findings: [
        {
          kind: "drop-table",
          statementIndex: 0,
          message: "DROP TABLE foo",
          affectedObject: "foo",
        },
      ],
      sql: "DROP TABLE foo",
      strict: false,
    });
    await screen.findByText(/Confirm destructive/);
    fireEvent.click(screen.getByText(/Run anyway/));
    expect(await promise).toBe(true);
  });

  it("standard mode: clicking Cancel resolves false", async () => {
    render(<DestructiveModalHost />);
    const promise = confirmDestructive({
      findings: [
        {
          kind: "truncate",
          statementIndex: 0,
          message: "...",
          affectedObject: "x",
        },
      ],
      sql: "TRUNCATE x",
      strict: false,
    });
    await screen.findByText(/Confirm destructive/);
    fireEvent.click(screen.getByText("Cancel"));
    expect(await promise).toBe(false);
  });

  it("strict mode: Run disabled until keyword typed", async () => {
    render(<DestructiveModalHost />);
    const promise = confirmDestructive({
      findings: [
        {
          kind: "drop-table",
          statementIndex: 0,
          message: "...",
          affectedObject: "x",
        },
      ],
      sql: "DROP TABLE x",
      strict: true,
    });
    await screen.findByText(/Confirm destructive/);
    const runBtn = screen.getByText("Run") as HTMLButtonElement;
    expect(runBtn.disabled).toBe(true);
    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "DROP" },
    });
    expect((screen.getByText("Run") as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByText("Run"));
    expect(await promise).toBe(true);
  });

  it("back-to-back identical requests cancel the prior", async () => {
    render(<DestructiveModalHost />);
    const args = {
      findings: [
        {
          kind: "drop-table",
          statementIndex: 0,
          message: "...",
          affectedObject: "x",
        } as const,
      ],
      sql: "DROP TABLE x",
      strict: true,
    };
    const first = confirmDestructive(args);
    // Don't await — fire the second immediately.
    const second = confirmDestructive(args);
    expect(await first).toBe(false);
    // Resolve the second one to clean up.
    await screen.findByText(/Confirm destructive/);
    fireEvent.click(screen.getByText("Cancel"));
    expect(await second).toBe(false);
  });
});
