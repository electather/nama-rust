import type { ReactElement } from "react";

import { useUnmatchedItems } from "@/features/admin-needs-a-match/data/use-unmatched-items";
import {
  type UnmatchedGroup,
  unmatchedGroup,
} from "@/features/admin-needs-a-match/lib/unmatched-group";
import type { Library } from "@/gen/nama/v1/library_pb";

/**
 * Loads `library`'s Unmatched Items, then those of the `rest` after it, and hands `children` every
 * Library's group, those `before` too, once they all have. Each Library is a component of its own,
 * since each pages through its list with a hook.
 */
export function LoadedUnmatched({
  library,
  rest: [next, ...rest],
  before,
  children,
}: {
  library: Library;
  rest: Library[];
  before: UnmatchedGroup[];
  children: (groups: UnmatchedGroup[]) => ReactElement;
}) {
  const { data, hasNextPage, totalSize, fetchNextPage } = useUnmatchedItems(library);
  const groups = [
    ...before,
    unmatchedGroup(library, { pages: data.pages, hasNextPage, totalSize, fetchNextPage }),
  ];
  return next === undefined ? (
    children(groups)
  ) : (
    <LoadedUnmatched library={next} rest={rest} before={groups}>
      {children}
    </LoadedUnmatched>
  );
}
