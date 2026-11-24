import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The query key of the image settings, to refetch after changing them. */
export function imageSettingsKey(transport: Transport) {
  return createConnectQueryKey({
    schema: ServerService.method.getImageSettings,
    cardinality: "finite",
    transport,
  });
}

/** The image settings. */
export function useImageSettings() {
  return useSuspenseQuery(ServerService.method.getImageSettings, {}, { staleTime: loaderStaleTime })
    .data;
}
