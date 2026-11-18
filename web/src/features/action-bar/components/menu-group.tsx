import { useId } from "react";

import { MenuRow } from "@/features/action-bar/components/menu-row";
import type { Group } from "@/features/action-bar/lib/menu";

/** A group of rows under its heading, the first being the list's `first`th row. */
export function MenuGroup({ group, first }: { group: Group; first: number }) {
  const id = useId();
  return (
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- a group of a combobox's options, which aren't <option>s
    <div role="group" aria-labelledby={id} className="p-1">
      <div id={id} className="px-2 py-1.5 text-xs font-medium text-muted-foreground">
        {group.heading}
      </div>
      {group.rows.map((row, index) => (
        <MenuRow key={row.key} row={row} index={first + index} />
      ))}
    </div>
  );
}
