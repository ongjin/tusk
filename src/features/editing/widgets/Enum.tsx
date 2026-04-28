import type { WidgetProps } from "./types";
import { SetNullButton } from "./SetNullButton";

export function EnumWidget({
  initial,
  nullable,
  onCommit,
  onCancel,
  enumValues,
  typeName,
}: WidgetProps & { enumValues: string[]; typeName: string }) {
  const cur =
    initial.kind === "Enum" ? initial.value.value : (enumValues[0] ?? "");
  return (
    <div className="flex items-center gap-1">
      <select
        autoFocus
        defaultValue={cur}
        onChange={(e) =>
          onCommit({ kind: "Enum", value: { typeName, value: e.target.value } })
        }
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onCancel();
          }
        }}
        className="bg-background border-input rounded-sm border px-2 py-1 text-xs"
      >
        {enumValues.map((v) => (
          <option key={v} value={v}>
            {v}
          </option>
        ))}
      </select>
      {nullable && <SetNullButton onCommit={onCommit} />}
    </div>
  );
}
