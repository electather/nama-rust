import { NothingListed } from "@/features/action-bar/components/nothing-listed";
import { RowGroup } from "@/features/action-bar/components/row-group";
import type { Bar } from "@/features/action-bar/hooks/use-action-bar";

/**
 * What the action bar lists: its groups of rows, or that there's nothing to list. While a search is
 * on its way the last results stay as they are.
 */
export function ActionBarList({ bar, listId }: { bar: Bar; listId: string }) {
  return (
    <div
      id={listId}
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- a <select> can't hold rows of art, badges and keys, nor leave focus in the field
      role="listbox"
      aria-label="Command menu"
      tabIndex={-1}
      // A press here mustn't take focus from the field: the typing goes on.
      onMouseDown={(event) => {
        event.preventDefault();
      }}
      className="max-h-105 min-h-0 scroll-py-1 scrollbar-quiet overflow-x-hidden overflow-y-auto outline-none"
    >
      {bar.active === undefined && <NothingListed query={bar.listed} />}
      {bar.groups.map((group) => (
        <RowGroup key={group.heading} group={group} bar={bar} listId={listId} />
      ))}
    </div>
  );
}
