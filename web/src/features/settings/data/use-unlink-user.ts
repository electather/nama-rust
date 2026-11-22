import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { userLinkKey } from "@/features/settings/data/use-user-link";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Unlinks the signed-in User from their Account, or stops showing their code. */
export function useUnlinkUser() {
  const queryClient = useQueryClient();
  return useMutation(UserService.method.unlinkUser, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: userLinkKey });
    },
  });
}
