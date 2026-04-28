import type { AiProvider } from "@/lib/types";
import { DEFAULT_GENERATION_MODELS } from "@/lib/ai/providers";

interface Props {
  provider: AiProvider;
  value: string;
  onChange: (v: string) => void;
}

export function ModelPicker({ provider, value, onChange }: Props) {
  const suggestions = DEFAULT_GENERATION_MODELS[provider];
  return (
    <div className="flex items-center gap-2">
      <input
        list={`models-${provider}`}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="border-input flex-1 rounded border px-2 py-1"
      />
      <datalist id={`models-${provider}`}>
        {suggestions.map((m) => (
          <option key={m} value={m} />
        ))}
      </datalist>
    </div>
  );
}
