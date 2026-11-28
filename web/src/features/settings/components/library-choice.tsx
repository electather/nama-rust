import { type ReactNode, useId } from "react";

import { LibraryFilter } from "@/features/admin-needs-a-match/components/library-filter";
import {
  type UnmatchedGroup,
  filesNeedingAMatch,
} from "@/features/admin-needs-a-match/lib/unmatched-group";
import { Segmented } from "@/features/settings/components/segmented";

/** A segment's `name`, with how many files it holds quieter beside it. */
function counted(name: string, count: number) {
  return (
    <>
      {name} <span className="ms-1 text-muted-foreground tabular-nums">{count}</span>
    </>
  );
}

/** Up to this many Libraries get a segment each; more are picked from a dropdown. */
const MOST_SEGMENTS = 4;

/**
 * Picks which Library's files show, or all (`onPick` with none): one segment for all and one each
 * for up to 4 Libraries, each with how many files it holds, else a dropdown. Nothing for one
 * Library, unless the `picked` one (an id) `missing` isn't on the Server.
 */
export function LibraryChoice({
  groups,
  picked,
  missing,
  onPick,
}: {
  groups: UnmatchedGroup[];
  picked: string | undefined;
  missing: boolean;
  onPick: (library: string | undefined) => void;
}) {
  const labelId = useId();
  if (groups.length < 2 && !missing) {
    return null;
  }
  if (groups.length > MOST_SEGMENTS || missing) {
    return (
      <LibraryFilter
        libraries={groups.map(({ library }) => library)}
        picked={picked}
        onPick={onPick}
      />
    );
  }
  const options: [string, ReactNode][] = [
    ["", counted("All", filesNeedingAMatch(groups))],
    ...groups.map(({ library, count }): [string, ReactNode] => [
      String(library.id),
      counted(library.name, count),
    ]),
  ];
  return (
    <div className="flex">
      <span id={labelId} className="sr-only">
        Library
      </span>
      <Segmented
        aria-labelledby={labelId}
        name="needs-a-match-library"
        options={options}
        value={picked ?? ""}
        onChange={(library) => {
          onPick(library === "" ? undefined : library);
        }}
      />
    </div>
  );
}
