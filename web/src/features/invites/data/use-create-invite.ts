import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { invitesKey } from "@/shared/hooks/invites-key";

/** Makes an Invite, then refetches the pending ones. */
export function useCreateInvite() {
  const queryClient = useQueryClient();
  return useMutation(UserService.method.createInvite, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: invitesKey });
    },
  });
}
