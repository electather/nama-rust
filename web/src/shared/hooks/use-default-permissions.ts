import { useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Permissions new Users get, which a new Invite starts from. */
export function useDefaultPermissions() {
  return useSuspenseQuery(
    UserService.method.getDefaultPermissions,
    {},
    { staleTime: loaderStaleTime },
  ).data.permissions;
}
