import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The query key of the Server's settings, to refetch after saving them. */
export const serverSettingsKey = createConnectQueryKey({
  schema: ServerService.method.getServerSettings,
  cardinality: "finite",
});

/** The Server's settings. */
export function useServerSettings() {
  return useSuspenseQuery(
    ServerService.method.getServerSettings,
    {},
    { staleTime: loaderStaleTime },
  ).data;
}
