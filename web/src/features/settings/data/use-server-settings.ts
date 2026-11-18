import type { Transport } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  createQueryOptions,
  useSuspenseQuery,
} from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";

/** The query key of the Server's settings, to refetch after saving them. */
export const serverSettingsKey = createConnectQueryKey({
  schema: ServerService.method.getServerSettings,
  cardinality: "finite",
});

/** The Server's settings, for Settings to load before it shows the form. */
export function serverSettingsQuery(transport: Transport) {
  return {
    ...createQueryOptions(ServerService.method.getServerSettings, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The Server's settings. */
export function useServerSettings() {
  return useSuspenseQuery(ServerService.method.getServerSettings, {}).data;
}
