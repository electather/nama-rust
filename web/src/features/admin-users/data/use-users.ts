import type { Transport } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  createQueryOptions,
  useSuspenseQuery,
} from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The query key of the Users, to refetch after a change to them. */
export const usersKey = createConnectQueryKey({
  schema: UserService.method.listUsers,
  cardinality: "finite",
});

/** The Users, for the Users page's loader to load before it shows. */
export function usersQuery(transport: Transport) {
  return {
    ...createQueryOptions(UserService.method.listUsers, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The Server's Users. */
export function useUsers() {
  return useSuspenseQuery(UserService.method.listUsers, {}).data.users;
}
