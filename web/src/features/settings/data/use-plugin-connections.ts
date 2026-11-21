import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { PluginConnectionState, UserService } from "@/gen/nama/v1/user_pb";

/** The query key of the Plugin Connections, to refetch after changing one. */
export const connectionsKey = createConnectQueryKey({
  schema: UserService.method.listPluginConnections,
  cardinality: "finite",
});

/** How often to look again while a link waits for the User to finish in the other app. */
const WAITING_POLL_MS = 2000;

/** The signed-in User's Plugin Connections, looked at again while one waits. */
export function usePluginConnections() {
  return useSuspenseQuery(
    UserService.method.listPluginConnections,
    {},
    {
      refetchInterval: (query) =>
        query.state.data?.connections.some(
          (connection) => connection.state === PluginConnectionState.WAITING,
        ) === true
          ? WAITING_POLL_MS
          : false,
    },
  ).data.connections;
}
