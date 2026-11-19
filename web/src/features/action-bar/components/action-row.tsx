import { Keys } from "@/features/action-bar/components/keys";
import { RowAction } from "@/features/action-bar/components/row-action";
import { RowLead } from "@/features/action-bar/components/row-lead";
import { RowText } from "@/features/action-bar/components/row-text";
import type { Bar } from "@/features/action-bar/hooks/use-action-bar";
import type { Row } from "@/features/action-bar/lib/rows";

/** Scrolls the row that has just become the selected one into the list's view. */
function keepInView(row: HTMLDivElement | null) {
  row?.scrollIntoView({ block: "nearest" });
}

/**
 * One option of the action bar's list. Focus never comes here: the field keeps it and names the
 * selected option with `aria-activedescendant`, so typing goes on while the arrows move.
 */
export function ActionRow({ row, bar, listId }: { row: Row; bar: Bar; listId: string }) {
  const selected = bar.active?.id === row.id;
  const handleClick = row.run;
  return (
    // oxlint-disable-next-line jsx-a11y/click-events-have-key-events -- the field takes the keys for every option (`handleFieldKeyDown`)
    <div
      ref={selected ? keepInView : undefined}
      id={`${listId}-${row.id}`}
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- an <option> can't hold art, a badge and keys
      role="option"
      aria-selected={selected}
      tabIndex={-1}
      onClick={handleClick}
      onMouseMove={() => {
        bar.pick(row.id);
      }}
      className="group relative flex min-h-17 cursor-default items-center gap-3 rounded-sm px-2.5 py-1.5 text-muted-foreground transition-colors select-none aria-selected:bg-accent aria-selected:text-foreground motion-reduce:transition-none"
    >
      <span
        aria-hidden
        className="absolute inset-y-2 inset-s-0 hidden w-0.5 rounded-full bg-primary group-aria-selected:block"
      />
      <RowLead row={row} />
      <RowText row={row} />
      <Keys keys={row.chips} />
      {selected && <RowAction action={row.action} />}
    </div>
  );
}
