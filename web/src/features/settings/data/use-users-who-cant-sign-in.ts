import { useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/**
 * The ids of the Server's Users who can't sign in: with no password, and no Linked Account to sign
 * in with, until an Admin gives them a password.
 */
export function useUsersWhoCantSignIn() {
  return useSuspenseQuery(UserService.method.listUsers, {}, { staleTime: loaderStaleTime }).data
    .cannotSignInUserIds;
}
