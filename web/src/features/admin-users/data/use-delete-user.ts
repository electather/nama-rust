import { useQueryClient } from "@tanstack/react-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";
import { usersKey } from "@/shared/hooks/use-users";

/** Deletes a User, then refetches the Users. */
export function useDeleteUser() {
  const queryClient = useQueryClient();
  return useSensitiveMutation(UserService.method.deleteUser, {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: usersKey });
    },
  });
}
