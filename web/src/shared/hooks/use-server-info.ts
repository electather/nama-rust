import { useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/**
 * What the Server the page's transport talks to says of itself. Every signed-in page has it
 * already, as its access check fetched it, but an error page may not reach the Server.
 */
export function useServerInfo() {
  return useSuspenseQuery(ServerService.method.getServerInfo, {}, { staleTime: loaderStaleTime })
    .data;
}
