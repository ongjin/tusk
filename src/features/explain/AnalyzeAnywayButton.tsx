import { Button } from "@/components/ui/button";
import { useTabs } from "@/store/tabs";

import { runExplainGate } from "./explainGate";

interface Props {
  tabId: string;
  connId: string;
  sql: string;
}

export function AnalyzeAnywayButton({ tabId, connId, sql }: Props) {
  const onClick = async () => {
    useTabs.getState().setBusy(tabId, true);
    const r = await runExplainGate({ connId, sql, allowAnalyzeAnyway: true });
    if (r) useTabs.getState().setPlan(tabId, r, sql);
    else useTabs.getState().setBusy(tabId, false);
  };
  return (
    <Button size="sm" variant="destructive" onClick={onClick}>
      ANALYZE anyway
    </Button>
  );
}
