import { useQueries } from "@tanstack/react-query";

import { homeRowsQuery } from "@/features/home/data/use-home-rows";
import { useReachedServers } from "@/features/home/data/use-reached-servers";
import { mergeRows } from "@/features/home/lib/hub-rows";

/** Home's rows at the Hub, each kind merged across the Servers that have answered so far. */
export function useHubRows() {
  const reached = useReachedServers();
  // Not suspended, as `useReachedServers` isn't.
  return useQueries({
    queries: reached.map(({ transport }) => homeRowsQuery(transport)),
    combine: (answers) =>
      mergeRows(
        reached.flatMap((server, at) => {
          const rows = answers[at]?.data?.rows;
          return rows ? [{ ...server, rows }] : [];
        }),
      ),
  });
}
