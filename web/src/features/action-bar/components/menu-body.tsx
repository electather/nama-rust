import { CatchBoundary } from "@tanstack/react-router";

import { SearchFailed } from "@/features/action-bar/components/search-failed";
import { SearchingList } from "@/features/action-bar/components/searching-list";
import type { ListState } from "@/features/action-bar/lib/menu";

/** The action bar's list for `state`, `listId`, saying so when its search has failed. */
export function MenuBody({ state, listId }: { state: ListState; listId: string }) {
  return (
    <div
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- a combobox's list, whose options hold art a <select> can't
      role="listbox"
      id={listId}
      aria-label="Results"
      className="max-h-105 scroll-py-1 overflow-x-hidden overflow-y-auto overscroll-contain"
    >
      <CatchBoundary
        getResetKey={() => `${state.frame}/${state.searched}`}
        errorComponent={SearchFailed}
      >
        <SearchingList state={state} />
      </CatchBoundary>
    </div>
  );
}
