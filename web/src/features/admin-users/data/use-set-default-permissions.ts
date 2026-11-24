import { createConnectQueryKey, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/** `SetDefaultPermissions`, after which the default Permissions are fresh. */
export function useSetDefaultPermissions() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useSensitiveMutation(UserService.method.setDefaultPermissions, {
    onSettled: async () => {
      await queryClient.invalidateQueries({
        queryKey: createConnectQueryKey({
          schema: UserService.method.getDefaultPermissions,
          transport,
          cardinality: "finite",
        }),
      });
    },
  });
}
