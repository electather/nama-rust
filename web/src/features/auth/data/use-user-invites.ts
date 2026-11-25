import { useSuspenseQuery } from "@connectrpc/connect-query";

import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/** The Users Admins created from the signed-in Account, waiting for it to accept them, oldest first. */
export function useUserInvites() {
  return useSuspenseQuery(LinkService.method.listUserInvites, {}).data.invites;
}
