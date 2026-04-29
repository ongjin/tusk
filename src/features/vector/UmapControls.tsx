import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface Props {
  sample: number;
  nNeighbors: number;
  minDist: number;
  onChange: (
    patch: Partial<{ sample: number; nNeighbors: number; minDist: number }>,
  ) => void;
  onRun: () => void;
  running: boolean;
}

export function UmapControls({
  sample,
  nNeighbors,
  minDist,
  onChange,
  onRun,
  running,
}: Props) {
  return (
    <div className="border-border bg-muted/20 flex flex-col gap-3 border-r p-3 text-xs">
      <div className="flex flex-col gap-1">
        <label className="text-muted-foreground" htmlFor="umap-sample">
          Sample size
        </label>
        <Input
          id="umap-sample"
          type="number"
          min={100}
          max={50000}
          value={sample}
          onChange={(e) => onChange({ sample: Number(e.target.value) })}
        />
      </div>
      <div className="flex flex-col gap-1">
        <label className="text-muted-foreground">n_neighbors: {nNeighbors}</label>
        <input
          aria-label="n_neighbors"
          type="range"
          min={2}
          max={100}
          value={nNeighbors}
          onChange={(e) => onChange({ nNeighbors: Number(e.target.value) })}
        />
      </div>
      <div className="flex flex-col gap-1">
        <label className="text-muted-foreground">min_dist: {minDist.toFixed(2)}</label>
        <input
          aria-label="min_dist"
          type="range"
          min={0}
          max={0.99}
          step={0.01}
          value={minDist}
          onChange={(e) => onChange({ minDist: Number(e.target.value) })}
        />
      </div>
      <Button onClick={onRun} disabled={running} size="sm">
        {running ? "Running…" : "Re-run UMAP"}
      </Button>
    </div>
  );
}
