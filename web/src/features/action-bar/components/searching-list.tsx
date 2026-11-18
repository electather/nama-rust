import { Suspense } from "react";

import { CommandList } from "@/features/action-bar/components/command-list";
import { Searching } from "@/features/action-bar/components/searching";
import type { ListState } from "@/features/action-bar/lib/menu";

/** The action bar's list for `state`, saying "Searching…" until its search first loads. */
export function SearchingList({ state }: { state: ListState }) {
  return (
    <Suspense fallback={<Searching />}>
      <CommandList state={state} />
    </Suspense>
  );
}
