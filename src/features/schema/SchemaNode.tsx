import { useState } from "react";
import { ChevronRight, ChevronDown } from "lucide-react";

import { cn } from "@/lib/utils";

interface Props {
  label: string;
  children: React.ReactNode;
  onExpand?: () => void;
  initiallyOpen?: boolean;
  indent?: number;
  onContextMenu?: (e: React.MouseEvent) => void;
}

export function SchemaNode({
  label,
  children,
  onExpand,
  initiallyOpen,
  indent = 0,
  onContextMenu,
}: Props) {
  const [open, setOpen] = useState(!!initiallyOpen);

  function toggle() {
    setOpen((prev) => {
      if (!prev) onExpand?.();
      return !prev;
    });
  }

  return (
    <div>
      <button
        type="button"
        className={cn(
          "hover:bg-accent flex w-full items-center gap-1 rounded px-1 py-0.5 text-left text-sm",
        )}
        style={{ paddingLeft: 4 + indent * 12 }}
        onClick={toggle}
        onContextMenu={onContextMenu}
      >
        {open ? (
          <ChevronDown className="size-3.5" />
        ) : (
          <ChevronRight className="size-3.5" />
        )}
        <span>{label}</span>
      </button>
      {open && <div>{children}</div>}
    </div>
  );
}
