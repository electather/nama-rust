import { useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";

/** The plays going on now, which the Playback events refetch. */
export function usePlays() {
  return useSuspenseQuery(ServerService.method.listPlays, {}).data.plays;
}
