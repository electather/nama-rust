import { useState } from "react";

import { EverythingMatched } from "@/features/admin-needs-a-match/components/everything-matched";
import { usePrefetchUnmatchedItems } from "@/features/admin-needs-a-match/data/use-unmatched-items";
import { UnmatchedLibrary } from "@/features/settings/components/unmatched-library";
import type { Library } from "@/gen/nama/v1/library_pb";

/** The Unmatched Items of each of `libraries` that has any, or that everything is Matched. */
export function UnmatchedLists({ libraries }: { libraries: Library[] }) {
  usePrefetchUnmatchedItems(libraries);
  // One Item at a time is being fixed, so one Search button is on screen.
  const [fixing, setFixing] = useState<bigint>();
  const toggle = (itemId: bigint) => {
    setFixing(fixing === itemId ? undefined : itemId);
  };
  const [first, ...rest] = libraries;
  if (first === undefined) {
    return <EverythingMatched anyListed={false} />;
  }
  return (
    <UnmatchedLibrary
      library={first}
      rest={rest}
      anyBefore={false}
      fixing={fixing}
      toggle={toggle}
    />
  );
}
