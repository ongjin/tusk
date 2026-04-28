import { classifyDestructive } from "@/lib/ai/destructive";
import { confirmDestructive } from "@/features/ai/DestructiveModal";
import { useSettings } from "@/store/settings";

/** Returns true when execution may proceed, false when user cancelled. */
export async function runGate(sql: string): Promise<boolean> {
  const findings = await classifyDestructive(sql);
  if (findings.length === 0) return true;
  const strict = useSettings.getState().destructiveStrict;
  return confirmDestructive({ findings, sql, strict });
}
