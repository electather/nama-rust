import { Keys } from "@/features/action-bar/components/keys";
import type { Row } from "@/features/action-bar/lib/rows";

/** A row's words: its label with its badge and keys, over its hint. */
export function RowText({ row }: { row: Row }) {
  return (
    <div className="flex min-w-0 flex-1 flex-col gap-0.5">
      <div className="flex min-w-0 items-center gap-2 text-sm font-medium text-foreground">
        <span className="truncate">{row.label}</span>
        {row.badge !== undefined && (
          <span className="inline-flex h-4.5 shrink-0 items-center rounded-full border bg-muted px-1.5 text-xs">
            {row.badge}
          </span>
        )}
        <Keys keys={row.keys} />
      </div>
      {row.hint !== "" && (
        // Quiet ink is too faint on a selected row's fill (DESIGN.md, "Neutral").
        <div className="truncate text-xs group-aria-selected:text-foreground">{row.hint}</div>
      )}
    </div>
  );
}
