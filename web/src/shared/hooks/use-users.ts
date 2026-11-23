import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The query key of the Users, to refetch after a change to them. */
export const usersKey = createConnectQueryKey({
  schema: UserService.method.listUsers,
  cardinality: "finite",
});

/** The Server's Users. */
export function useUsers() {
  return useSuspenseQuery(UserService.method.listUsers, {}, { staleTime: loaderStaleTime }).data
    .users;
}

/**
 * The ids of the Server's Users who can't sign in: with no password, and no Linked Account to sign
 * in with, until an Admin gives them a password.
 */
export function useUsersWhoCantSignIn() {
  return useSuspenseQuery(UserService.method.listUsers, {}, { staleTime: loaderStaleTime }).data
    .cannotSignInUserIds;
}
