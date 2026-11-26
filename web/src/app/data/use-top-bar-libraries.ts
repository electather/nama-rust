import { useQuery } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";

import { useReachedServers } from "@/features/servers/data/use-reached-servers";
import { type LinkedServer, LinkService } from "@/gen/nama/hub/v1/link_pb";
import { type Library, LibraryService } from "@/gen/nama/v1/library_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";
import { librariesQuery } from "@/shared/hooks/use-libraries";

/** The Libraries on one Server, which at the Hub names it. */
export interface LibraryGroup {
  server?: LinkedServer | undefined;
  libraries: Library[];
}

/**
 * The Libraries the top bar links, without suspending: this Server's, or at the Hub each reached
 * Server's, as each loads. A Server whose Libraries haven't loaded, or failed to, isn't listed.
 */
export function useTopBarLibraries(atHub: boolean): LibraryGroup[] {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the bar shows without its Libraries while they load or if they fail
  const own = useQuery(
    LibraryService.method.listLibraries,
    {},
    { staleTime: loaderStaleTime, enabled: !atHub, select: ({ libraries }) => [{ libraries }] },
  );
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- as above, and a Server has no Hub to ask
  const linked = useQuery(
    LinkService.method.listLinkedServers,
    {},
    { staleTime: loaderStaleTime, enabled: atHub, select: ({ servers }) => servers },
  );
  const { reached } = useReachedServers(linked.data ?? []);
  const theirs = useQueries({
    queries: reached.map(({ server, transport }) =>
      Object.assign(librariesQuery(transport), {
        select: ({ libraries }: { libraries: Library[] }) => ({ server, libraries }),
      }),
    ),
    combine: (results) => results.flatMap(({ data }) => (data ? [data] : [])),
  });
  return (atHub ? theirs : own.data) ?? [];
}
