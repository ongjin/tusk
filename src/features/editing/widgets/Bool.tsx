import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

export function BoolWidget({ initial, nullable, onCommit }: WidgetProps) {
  const cur = initial.kind === "Bool" ? initial.value : false;
  return (
    <div className="flex items-center gap-2">
      <input
        type="checkbox"
        defaultChecked={cur}
        autoFocus
        onChange={(e) =>
          onCommit({ kind: "Bool", value: e.currentTarget.checked })
        }
      />
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
