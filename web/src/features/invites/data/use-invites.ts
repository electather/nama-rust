import { useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The pending Invites, and whether the Server has a Server URL to link them to. */
export function useInvites() {
  return useSuspenseQuery(UserService.method.listInvites, {}).data;
}
