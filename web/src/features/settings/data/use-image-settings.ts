import type { Transport } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  createQueryOptions,
  useSuspenseQuery,
} from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The query key of the image settings, to refetch after changing them. */
export const imageSettingsKey = createConnectQueryKey({
  schema: ServerService.method.getImageSettings,
  cardinality: "finite",
});

/** The image settings, for Settings to load before it shows the form. */
export function imageSettingsQuery(transport: Transport) {
  return {
    ...createQueryOptions(ServerService.method.getImageSettings, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The image settings. */
export function useImageSettings() {
  return useSuspenseQuery(ServerService.method.getImageSettings, {}, { staleTime: loaderStaleTime })
    .data;
}
