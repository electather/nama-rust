import { useQueries } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";

import { serverReachQuery } from "@/features/servers/data/server-reach";
import type { LibraryKind } from "@/gen/nama/v1/library_pb";
import { librariesQuery } from "@/shared/hooks/use-libraries";

/**
 * At the Hub, the Libraries of `kind` on each of the Account's other Servers, signed in there, as
 * each answers. None on a Server's own web app.
 */
export function useLibrariesElsewhere(kind: LibraryKind) {
  const { elsewhere } = useRouteContext({ from: "/_member/_server" });
  // Not suspended, so a slow or unreachable Server holds up neither the page nor the rest.
  const reached = useQueries({
    queries: elsewhere.map((reach) => serverReachQuery(reach)),
    combine: (reaches) =>
      elsewhere.flatMap(({ server }, at) => {
        const reach = reaches[at]?.data;
        return reach?.kind === "signed-in" ? [{ server, transport: reach.transport }] : [];
      }),
  });
  return useQueries({
    queries: reached.map(({ transport }) => librariesQuery(transport)),
    combine: (lists) =>
      reached.flatMap(({ server, transport }, at) =>
        (lists[at]?.data?.libraries ?? [])
          .filter((library) => library.kind === kind)
          .map(({ id }) => ({ server, transport, libraryId: id })),
      ),
  });
}
