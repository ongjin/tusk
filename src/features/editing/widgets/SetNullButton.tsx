import type { Cell } from "@/lib/types";

export function SetNullButton({ onCommit }: { onCommit: (c: Cell) => void }) {
  return (
    <button
      type="button"
      onClick={() => onCommit({ kind: "Null" })}
      className="border-input hover:bg-accent rounded border px-1 text-xs"
    >
      Set NULL
    </button>
  );
}
