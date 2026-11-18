import { useState } from "react";

import { EverythingMatched } from "@/features/admin-needs-a-match/components/everything-matched";
import { LibraryList } from "@/features/admin-needs-a-match/components/library-list";
import type { Library } from "@/gen/nama/v1/library_pb";

/** The Unmatched Items of each of `libraries` that has any, or that everything is Matched. */
export function UnmatchedLists({ libraries: [first, ...rest] }: { libraries: Library[] }) {
  // One Item at a time is being fixed, so one Search button is on screen.
  const [fixing, setFixing] = useState<bigint>();
  const toggle = (itemId: bigint) => {
    setFixing(fixing === itemId ? undefined : itemId);
  };
  if (first === undefined) {
    return <EverythingMatched anyListed={false} />;
  }
  return (
    <LibraryList library={first} rest={rest} anyBefore={false} fixing={fixing} toggle={toggle} />
  );
}
