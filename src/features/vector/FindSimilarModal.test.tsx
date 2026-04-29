import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FindSimilarModal } from "./FindSimilarModal";

describe("FindSimilarModal", () => {
  it("renders SQL preview matching the operator", () => {
    render(
      <FindSimilarModal
        open={{
          connId: "c1",
          schema: "public",
          table: "items",
          vecCol: "embedding",
          pkCols: ["id"],
          queryVector: [0.1, 0.2],
        }}
        onClose={() => {}}
      />,
    );
    expect(
      screen.getByText(/embedding" <=> '\[0.1,0.2\]'::vector/),
    ).toBeInTheDocument();
  });

  it("changes operator updates preview", () => {
    render(
      <FindSimilarModal
        open={{
          connId: "c1",
          schema: "public",
          table: "items",
          vecCol: "embedding",
          pkCols: ["id"],
          queryVector: [0.1],
        }}
        onClose={() => {}}
      />,
    );
    const select = screen.getByRole("combobox");
    fireEvent.change(select, { target: { value: "<->" } });
    const pre = screen.getByText(
      (content) => content.includes("<->") && !content.includes("—"),
    );
    expect(pre).toBeInTheDocument();
  });
});
