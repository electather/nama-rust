import { KeyChips } from "@/features/action-bar/components/key-chips";
import { RowAffordance } from "@/features/action-bar/components/row-affordance";
import { RowArt } from "@/features/action-bar/components/row-art";
import { RowText } from "@/features/action-bar/components/row-text";
import { useSelection } from "@/features/action-bar/hooks/use-selection";
import type { Row } from "@/features/action-bar/lib/menu";

/**
 * One option of the action bar's list, the `index`th, selected on hover and picked on click, as
 * Enter picks it. Pressing it leaves focus in the field.
 */
export function MenuRow({ row, index }: { row: Row; index: number }) {
  const { idPrefix, selected, onSelect, list } = useSelection();
  const isSelected = index === selected;
  return (
    // oxlint-disable-next-line jsx-a11y/click-events-have-key-events -- the field's keys select and pick rows, as a combobox's
    <div
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- an <option> can't hold art
      role="option"
      id={`${idPrefix}-${index}`}
      tabIndex={-1}
      aria-selected={isSelected}
      onMouseDown={(event) => {
        event.preventDefault();
      }}
      onClick={() => {
        list.current?.pick(index);
      }}
      onMouseMove={() => {
        if (!isSelected) {
          onSelect(index);
        }
      }}
      className="group flex cursor-default items-center gap-3 rounded-sm px-2.5 py-2 text-muted-foreground transition-colors duration-150 ease-out-expo select-none aria-selected:bg-accent aria-selected:text-foreground motion-reduce:transition-none"
    >
      <RowArt art={row.art} />
      <RowText row={row} />
      {row.chips !== undefined && <KeyChips keys={row.chips} />}
      {isSelected && row.affordance !== undefined && <RowAffordance label={row.affordance} />}
    </div>
  );
}
