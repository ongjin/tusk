import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { buildAnnSql } from "@/lib/vector/annSql";
import {
  ANN_OPERATOR_LABELS,
  type AnnOperator,
} from "@/lib/vector/types";
import { useTabs } from "@/store/tabs";

export interface FindSimilarOpen {
  connId: string;
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  queryVector: number[];
}

interface Props {
  open: FindSimilarOpen | null;
  onClose: () => void;
}

export function FindSimilarModal({ open, onClose }: Props) {
  const [op, setOp] = useState<AnnOperator>("<=>");
  const [limit, setLimit] = useState<number>(20);

  const sql = useMemo(() => {
    if (!open) return "";
    return buildAnnSql({
      schema: open.schema,
      table: open.table,
      vecCol: open.vecCol,
      pkCols: open.pkCols,
      queryVector: open.queryVector,
      op,
      limit,
    });
  }, [open, op, limit]);

  if (!open) return null;

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Find similar rows</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3 text-sm">
          <div className="flex items-center gap-2">
            <label className="text-muted-foreground w-24 text-xs">
              Operator
            </label>
            <select
              value={op}
              onChange={(e) => setOp(e.target.value as AnnOperator)}
              className="border-input rounded border bg-transparent px-2 py-1 text-xs"
            >
              {(Object.keys(ANN_OPERATOR_LABELS) as AnnOperator[]).map((k) => (
                <option key={k} value={k}>
                  {k} — {ANN_OPERATOR_LABELS[k]}
                </option>
              ))}
            </select>
          </div>
          <div className="flex items-center gap-2">
            <label className="text-muted-foreground w-24 text-xs">LIMIT</label>
            <Input
              type="number"
              min={1}
              max={10000}
              value={limit}
              onChange={(e) => setLimit(Number(e.target.value))}
              className="w-32"
            />
          </div>
          <div>
            <div className="text-muted-foreground mb-1 text-xs">SQL</div>
            <pre className="bg-muted max-h-64 overflow-auto rounded p-2 text-[11px]">
              {sql}
            </pre>
          </div>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            onClick={() => {
              const t = useTabs.getState();
              const id = t.newTab(open.connId);
              t.updateSql(id, sql);
              t.setActive(id);
              t.requestRun();
              onClose();
            }}
          >
            Run
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
