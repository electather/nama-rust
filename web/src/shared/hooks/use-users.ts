import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The query key of the Users, to refetch after a change to them. */
export function usersKey(transport: Transport) {
  return createConnectQueryKey({
    schema: UserService.method.listUsers,
    cardinality: "finite",
    transport,
  });
}

/** The Server's Users. */
export function useUsers() {
  return useSuspenseQuery(UserService.method.listUsers, {}, { staleTime: loaderStaleTime }).data
    .users;
}
