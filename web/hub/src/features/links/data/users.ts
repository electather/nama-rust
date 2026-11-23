import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { serversKey } from "@hub/features/links/data/servers";
import { LinkService } from "@hub/gen/nama/hub/v1/link_pb";
import { useQueryClient } from "@tanstack/react-query";

/** The query key of the signed-in Account's Linked Users. */
export const usersKey = createConnectQueryKey({
  schema: LinkService.method.listLinkedUsers,
  cardinality: "finite",
});

/**
 * The signed-in Account's Linked Users, oldest first. It doesn't suspend, as the Servers' list
 * doesn't: the rest of the Account page works without it.
 */
export function useLinkedUsers() {
  return useQuery(LinkService.method.listLinkedUsers, {});
}

/**
 * Links the User whose Server shows a code to the signed-in Account, then lists the Users and
 * Servers again.
 */
export function useApproveUserLink() {
  const queryClient = useQueryClient();
  return useMutation(LinkService.method.approveUserLink, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: usersKey });
      await queryClient.invalidateQueries({ queryKey: serversKey });
    },
  });
}

/** Unlinks one of the signed-in Account's Users, then lists the Users and Servers again. */
export function useUnlinkLinkedUser() {
  const queryClient = useQueryClient();
  return useMutation(LinkService.method.unlinkLinkedUser, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: usersKey });
      await queryClient.invalidateQueries({ queryKey: serversKey });
    },
  });
}
