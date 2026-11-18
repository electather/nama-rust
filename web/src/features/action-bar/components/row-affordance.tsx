import { CornerDownLeft } from "lucide-react";

import { Kbd } from "@/features/action-bar/components/kbd";

/** What Enter does on the selected row, such as "Open". */
export function RowAffordance({ label }: { label: string }) {
  return (
    <span className="ms-auto inline-flex shrink-0 items-center gap-1.5 text-xs text-foreground-soft">
      {label}
      <Kbd bordered>
        <CornerDownLeft aria-hidden strokeWidth={1.5} className="size-3" />
      </Kbd>
    </span>
  );
}
