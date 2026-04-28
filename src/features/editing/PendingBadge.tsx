import { Button } from "@/components/ui/button";
import { usePendingChanges } from "@/store/pendingChanges";

interface Props {
  onPreview: () => void;
  onSubmit: () => void;
  onRevert: () => void;
}

export function PendingBadge({ onPreview, onSubmit, onRevert }: Props) {
  const count = usePendingChanges((s) => s.byRow.size);
  if (count === 0) return null;

  return (
    <div className="flex items-center gap-1.5">
      <span
        className="inline-flex items-center rounded-md bg-amber-500/15 px-2 py-0.5 text-xs font-medium text-amber-600 dark:text-amber-400"
        title={`${count} row${count === 1 ? "" : "s"} pending`}
      >
        {count} pending
      </span>
      <Button size="xs" variant="outline" onClick={onPreview}>
        Preview
      </Button>
      <Button size="xs" variant="default" onClick={onSubmit}>
        Submit
      </Button>
      <Button size="xs" variant="ghost" onClick={onRevert}>
        Revert
      </Button>
    </div>
  );
}
