import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { devicesKey } from "@/shared/hooks/devices-key";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";
import { usersKey } from "@/shared/hooks/use-users";

/** Sets a User's password, then refetches the Users and their Devices. */
export function useResetPassword() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useSensitiveMutation(UserService.method.resetPassword, {
    // Their Devices were signed out, or they were deleted meanwhile.
    onSettled: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: devicesKey(transport) }),
        queryClient.invalidateQueries({ queryKey: usersKey(transport) }),
      ]);
    },
  });
}
