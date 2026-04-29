import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";

import { PlanNodeDetail } from "./PlanNodeDetail";

describe("PlanNodeDetail", () => {
  it("renders 'click a node to inspect' when null", () => {
    render(<PlanNodeDetail node={null} planOnly={false} />);
    expect(screen.getByText(/Click a node/i)).toBeInTheDocument();
  });

  it("renders relation + filter + time", () => {
    render(
      <PlanNodeDetail
        node={{
          nodeType: "Seq Scan",
          relationName: "users",
          schema: "public",
          startupCost: 0,
          totalCost: 50,
          planRows: 50,
          planWidth: 32,
          actualStartupTime: 0,
          actualTotalTime: 8,
          actualLoops: 1,
          actualRows: 50,
          rowsRemovedByFilter: 0,
          filter: "(email = 'a')",
          buffers: null,
          children: [],
          selfMs: 8,
          selfTimeRatio: 8 / 12,
          selfCostRatio: 0.5,
        }}
        planOnly={false}
      />,
    );
    expect(screen.getByText(/public.users/)).toBeInTheDocument();
    expect(screen.getByText(/email = 'a'/)).toBeInTheDocument();
    expect(screen.getByText(/8\.00 ms/)).toBeInTheDocument();
  });
});
