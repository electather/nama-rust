import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The pending Invites, for the Users page's loader to load before it shows. */
export function invitesQuery(transport: Transport) {
  return {
    ...createQueryOptions(UserService.method.listInvites, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The pending Invites, and whether the Server has a Server URL to link them to. */
export function useInvites() {
  return useSuspenseQuery(UserService.method.listInvites, {}).data;
}
