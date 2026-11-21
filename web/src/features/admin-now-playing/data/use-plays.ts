import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The plays going on now, for the Now Playing page's loader to load before it shows. */
export function playsQuery(transport: Transport) {
  return createQueryOptions(ServerService.method.listPlays, {}, { transport });
}

/** The plays `playsQuery` loads, which the Playback events refetch. */
export function usePlays() {
  return useSuspenseQuery(ServerService.method.listPlays, {}, { staleTime: loaderStaleTime }).data
    .plays;
}
