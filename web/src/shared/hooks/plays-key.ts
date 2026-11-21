import { createConnectQueryKey } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";

/** The query key of the plays going on now, which the Playback events refetch. */
export const playsKey = createConnectQueryKey({
  schema: ServerService.method.listPlays,
  cardinality: "finite",
});
