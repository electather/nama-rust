import { useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The pending Invites, and whether the Server has a Server URL to link them to. */
export function useInvites() {
  return useSuspenseQuery(UserService.method.listInvites, {}, { staleTime: loaderStaleTime }).data;
}
