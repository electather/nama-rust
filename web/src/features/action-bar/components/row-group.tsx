import { useId } from "react";

import { ActionRow } from "@/features/action-bar/components/action-row";
import type { Bar } from "@/features/action-bar/hooks/use-action-bar";
import type { Group } from "@/features/action-bar/lib/rows";

/** The rows under one heading of the action bar's list. */
export function RowGroup({ group, bar, listId }: { group: Group; bar: Bar; listId: string }) {
  const headingId = useId();
  return (
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- an <optgroup> only groups a <select>'s options
    <div role="group" aria-labelledby={headingId} className="p-1">
      <div id={headingId} className="px-2 py-1.5 text-xs font-medium text-muted-foreground">
        {group.heading}
      </div>
      {group.rows.map((row) => (
        <ActionRow key={row.id} row={row} bar={bar} listId={listId} />
      ))}
    </div>
  );
}
