import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { UmapControls } from "./UmapControls";

describe("UmapControls", () => {
  it("calls onChange with new sample size", () => {
    const onChange = vi.fn();
    render(
      <UmapControls
        sample={10000}
        nNeighbors={15}
        minDist={0.1}
        onChange={onChange}
        onRun={() => {}}
        running={false}
      />,
    );
    const input = screen.getByLabelText(/sample/i);
    fireEvent.change(input, { target: { value: "5000" } });
    expect(onChange).toHaveBeenCalledWith({ sample: 5000 });
  });

  it("Run button disabled when running", () => {
    render(
      <UmapControls
        sample={10000}
        nNeighbors={15}
        minDist={0.1}
        onChange={() => {}}
        onRun={() => {}}
        running={true}
      />,
    );
    expect(screen.getByRole("button", { name: /run/i })).toBeDisabled();
  });
});
