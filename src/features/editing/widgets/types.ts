import type { Cell } from "@/lib/types";

export interface WidgetProps {
  initial: Cell;
  nullable: boolean;
  onCommit: (next: Cell) => void;
  onCancel: () => void;
}
