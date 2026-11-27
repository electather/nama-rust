import { useQueries } from "@tanstack/react-query";

import { homeRowsQuery } from "@/features/home/data/use-home-rows";
import { mergeRows } from "@/features/home/lib/hub-rows";
import { useReachedServers } from "@/features/servers/data/use-reached-servers";
import { useLinkedServers } from "@/shared/hooks/use-linked-servers";

/** Home's rows at the Hub, each kind merged across the Servers that have answered so far. */
export function useHubRows() {
  const { reached } = useReachedServers(useLinkedServers());
  // Not suspended, as the servers feature's `useReachedServers` isn't.
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
