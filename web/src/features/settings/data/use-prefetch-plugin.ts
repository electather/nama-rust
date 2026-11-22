import { createQueryOptions, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/**
 * Starts loading a Plugin's details as soon as the User points at it or tabs to it on the list, so
 * its view usually opens with them. One fetched in the last `loaderStaleTime` isn't asked for again.
 */
export function usePrefetchPlugin() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return async (id: string) => {
    try {
      await queryClient.query({
        ...createQueryOptions(PluginService.method.getPlugin, { id }, { transport }),
        staleTime: loaderStaleTime,
      });
    } catch {
      // Opening the Plugin asks again and shows why.
    }
  };
}
