import { useEffect, useRef } from "react";

import { SetNullButton } from "./SetNullButton";
import type { WidgetProps } from "./types";

export function BoolWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
}: WidgetProps) {
  const isNull = initial.kind === "Null";
  const cur = initial.kind === "Bool" ? initial.value : false;
  const checkboxRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Indeterminate visual when starting from NULL.
    if (checkboxRef.current) {
      checkboxRef.current.indeterminate = isNull;
    }
  }, [isNull]);

  return (
    <div
      className="flex items-center gap-2"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          onCancel();
        }
      }}
    >
      <input
        type="checkbox"
        ref={checkboxRef}
        defaultChecked={cur}
        autoFocus={!isNull}
        onChange={(e) =>
          onCommit({ kind: "Bool", value: e.currentTarget.checked })
        }
      />
      {isNull && (
        <span className="text-muted-foreground text-xs italic">(was NULL)</span>
      )}
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
