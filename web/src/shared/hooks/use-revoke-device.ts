import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { devicesKey } from "@/shared/hooks/devices-key";

/** Revokes a Device, then refetches the Devices, refused or not as the list may be out of date. */
export function useRevokeDevice() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(UserService.method.revokeDevice, {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: devicesKey(transport) });
    },
  });
}
