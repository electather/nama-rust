import { useEffect, useImperativeHandle } from "react";

import { MenuGroup } from "@/features/action-bar/components/menu-group";
import { NoResults } from "@/features/action-bar/components/no-results";
import { Searching } from "@/features/action-bar/components/searching";
import { useFoundTitles } from "@/features/action-bar/data/use-found-titles";
import { useSelection } from "@/features/action-bar/hooks/use-selection";
import type { ListState } from "@/features/action-bar/lib/menu";
import { firstIndexes, isSearching, menuGroups } from "@/features/action-bar/lib/menu-groups";

/**
 * The groups of rows the action bar lists for `state`. While a search loads, what the last one
 * found stays up, under "Searching…".
 */
export function CommandList({ state }: { state: ListState }) {
  const found = useFoundTitles(state.searched);
  const groups = menuGroups({ ...state, found });
  const rows = groups.flatMap((group) => group.rows);
  const { list, onPick, selected, onSelect } = useSelection();
  useEffect(() => {
    if (selected >= rows.length) {
      onSelect(Math.max(rows.length - 1, 0));
    }
  }, [selected, rows.length, onSelect]);
  useImperativeHandle(
    list,
    () => ({
      count: rows.length,
      pick: (index) => {
        const row = rows[index];
        if (row !== undefined) {
          onPick(row.pick);
        }
      },
    }),
    [rows, onPick],
  );
  const searching = isSearching(state);
  const firsts = firstIndexes(groups);
  return (
    <>
      {searching && <Searching />}
      {rows.length === 0 && !searching && <NoResults typed={state.typed} />}
      {groups.map((group, index) => (
        <MenuGroup key={group.heading} group={group} first={firsts[index] ?? 0} />
      ))}
    </>
  );
}
