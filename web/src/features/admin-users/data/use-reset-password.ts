import { useQueryClient } from "@tanstack/react-query";

import { usersKey } from "@/features/admin-users/data/use-users";
import { UserService } from "@/gen/nama/v1/user_pb";
import { devicesKey } from "@/lib/users";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/** Sets a User's password, then refetches the Users and their Devices. */
export function useResetPassword() {
  const queryClient = useQueryClient();
  return useSensitiveMutation(UserService.method.resetPassword, {
    // Their Devices were signed out, or they were deleted meanwhile.
    onSettled: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: devicesKey }),
        queryClient.invalidateQueries({ queryKey: usersKey }),
      ]);
    },
  });
}
