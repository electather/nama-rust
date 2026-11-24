import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { devicesKey } from "@/shared/hooks/devices-key";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/** Changes the signed-in User's own password, then refetches their Devices. */
export function useChangePassword() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useSensitiveMutation(UserService.method.changePassword, {
    // The Server signed out every other Device.
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: devicesKey(transport) });
    },
  });
}
