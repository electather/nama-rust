import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { usersKey } from "@/shared/hooks/use-users";

/** Creates a User from an Account, then refetches the Users. */
export function useCreateUserFromAccount() {
  const queryClient = useQueryClient();
  return useMutation(UserService.method.createUserFromAccount, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: usersKey });
    },
  });
}
