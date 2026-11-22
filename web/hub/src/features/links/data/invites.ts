import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { usersKey } from "@hub/features/links/data/users";
import { LinkService } from "@hub/gen/nama/hub/v1/link_pb";
import { useQueryClient } from "@tanstack/react-query";

const invitesKey = createConnectQueryKey({
  schema: LinkService.method.listUserInvites,
  cardinality: "finite",
});

/**
 * The Users Admins created from the signed-in Account, waiting for it to accept them. It doesn't
 * suspend: the rest of the Account page works without it.
 */
export function useUserInvites() {
  return useQuery(LinkService.method.listUserInvites, {});
}

/** Accepts a User created from the signed-in Account, then lists its invites and Users again. */
export function useAcceptUserInvite() {
  const queryClient = useQueryClient();
  return useMutation(LinkService.method.acceptUserInvite, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: invitesKey });
      await queryClient.invalidateQueries({ queryKey: usersKey });
    },
  });
}
