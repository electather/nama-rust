import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { invitesKey } from "@/shared/hooks/invites-key";

/** Revokes an Invite, then refetches the pending ones. */
export function useRevokeInvite() {
  const queryClient = useQueryClient();
  return useMutation(UserService.method.revokeInvite, {
    // Refused or not, the list may be out of date.
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: invitesKey });
    },
  });
}
