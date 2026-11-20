import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The default Permissions, for the Users page's loader to load before it shows. */
export function defaultPermissionsQuery(transport: Transport) {
  return {
    ...createQueryOptions(UserService.method.getDefaultPermissions, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The Permissions new Users get, which a new Invite starts from. */
export function useDefaultPermissions() {
  return useSuspenseQuery(
    UserService.method.getDefaultPermissions,
    {},
    { staleTime: loaderStaleTime },
  ).data.permissions;
}
