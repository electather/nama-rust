import { KeyChips } from "@/features/action-bar/components/key-chips";
import type { Row } from "@/features/action-bar/lib/menu";

/** A row's label, with its badge and hotkey, over its hint. */
export function RowText({ row }: { row: Row }) {
  return (
    <div className="flex min-w-0 flex-1 flex-col gap-0.5">
      <div className="flex min-w-0 items-center gap-2 text-sm font-medium text-foreground">
        <span className="truncate">{row.label}</span>
        {row.badge !== undefined && (
          <span className="inline-flex h-4.5 shrink-0 items-center rounded-full border bg-muted px-1.5 text-xs leading-none font-medium text-foreground-soft">
            {row.badge}
          </span>
        )}
        {row.hotkey !== undefined && <KeyChips keys={row.hotkey} />}
      </div>
      {row.hint !== undefined && (
        <div className="truncate text-xs text-muted-foreground group-aria-selected:text-foreground-soft">
          {row.hint}
        </div>
      )}
    </div>
  );
}
