import type { ErrorComponentProps } from "@tanstack/react-router";
import { useImperativeHandle } from "react";

import { MenuGroup } from "@/features/action-bar/components/menu-group";
import { useResetSearchError } from "@/features/action-bar/data/use-reset-search-error";
import { useSelection } from "@/features/action-bar/hooks/use-selection";
import type { Group } from "@/features/action-bar/lib/menu";

const failed: Group = {
  heading: "Search failed",
  rows: [
    {
      key: "search:retry",
      label: "Retry",
      hint: "Couldn’t reach the Server. Check your connection and try again.",
      art: { icon: "refresh" },
      pick: { kind: "nothing" },
    },
  ],
};

/** What the action bar lists when its search fails: a row to search again. */
export function SearchFailed({ reset }: ErrorComponentProps) {
  const resetSearchError = useResetSearchError();
  const { list } = useSelection();
  useImperativeHandle(
    list,
    () => ({
      count: 1,
      pick: () => {
        resetSearchError();
        reset();
      },
    }),
    [reset, resetSearchError],
  );
  return <MenuGroup group={failed} first={0} />;
}
