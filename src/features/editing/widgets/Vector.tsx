import type { WidgetProps } from "./types";

export function VectorWidget({ initial, onCancel }: WidgetProps) {
  if (initial.kind !== "Vector") return null;
  return (
    <div className="flex items-center gap-2 text-xs">
      <span className="text-muted-foreground italic">
        vector({initial.value.dim}) — read-only in this version
      </span>
      <button
        type="button"
        onClick={onCancel}
        className="rounded-sm border px-2 py-1 text-xs"
      >
        Close
      </button>
    </div>
  );
}
