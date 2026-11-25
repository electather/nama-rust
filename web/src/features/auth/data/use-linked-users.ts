import { createConnectQueryKey, useSuspenseQuery, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/** The signed-in Account's Linked Users, oldest first. */
export function useLinkedUsers() {
  return useSuspenseQuery(LinkService.method.listLinkedUsers, {}).data.users;
}

/**
 * Lists the Account's Servers, Users and Invites again, after a change to any of them: Linking or
 * Unlinking a User adds a Server or takes it away, and accepting an Invite Links its User.
 */
export function useRefreshLinks() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return async () => {
    await queryClient.invalidateQueries({
      queryKey: createConnectQueryKey({ schema: LinkService, cardinality: undefined, transport }),
    });
  };
}
