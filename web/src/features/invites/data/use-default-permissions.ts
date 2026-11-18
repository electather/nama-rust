import { useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The Permissions new Users get, which a new Invite starts from. */
export function useDefaultPermissions() {
  return useSuspenseQuery(UserService.method.getDefaultPermissions, {}).data.permissions;
}
