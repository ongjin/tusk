import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { PlanTree } from "./PlanTree";
import type { PlanNode } from "@/lib/explain/planTypes";

const tree: PlanNode = {
  nodeType: "Hash Join",
  startupCost: 0,
  totalCost: 100,
  planRows: 100,
  planWidth: 32,
  actualStartupTime: 0,
  actualTotalTime: 12,
  actualLoops: 1,
  actualRows: 100,
  rowsRemovedByFilter: null,
  buffers: null,
  children: [
    {
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
      buffers: null,
      children: [],
      selfMs: 8,
      selfTimeRatio: 8 / 12,
      selfCostRatio: 0.5,
    },
  ],
  selfMs: 4,
  selfTimeRatio: 4 / 12,
  selfCostRatio: 0.5,
};

describe("PlanTree", () => {
  it("renders all nodes", () => {
    render(
      <PlanTree
        root={tree}
        selectedPath={[]}
        onSelect={() => {}}
        planOnly={false}
      />,
    );
    expect(screen.getByText(/Hash Join/)).toBeInTheDocument();
    expect(screen.getByText(/Seq Scan/)).toBeInTheDocument();
  });

  it("calls onSelect with the right path", () => {
    const onSelect = vi.fn();
    render(
      <PlanTree
        root={tree}
        selectedPath={[]}
        onSelect={onSelect}
        planOnly={false}
      />,
    );
    fireEvent.click(screen.getByText(/Seq Scan/));
    expect(onSelect).toHaveBeenCalledWith([0]);
  });

  it("flags heavy node with ⚠", () => {
    render(
      <PlanTree
        root={tree}
        selectedPath={[]}
        onSelect={() => {}}
        planOnly={false}
      />,
    );
    expect(screen.getAllByText(/⚠/).length).toBeGreaterThan(0);
  });
});
