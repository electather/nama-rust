import { useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";

/** The Server's name and setup state, fetched already to know whether it needs setting up. */
export function useServerInfo() {
  return useSuspenseQuery(ServerService.method.getServerInfo, {}).data;
}
