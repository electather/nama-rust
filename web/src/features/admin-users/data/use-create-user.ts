import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { usersKey } from "@/features/admin-users/data/use-users";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Creates a User, then refetches the Users. */
export function useCreateUser() {
  const queryClient = useQueryClient();
  return useMutation(UserService.method.createUser, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: usersKey });
    },
  });
}
