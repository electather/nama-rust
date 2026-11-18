import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { connectionsKey } from "@/features/settings/data/use-plugin-connections";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Deletes a connection to a Plugin, then refetches the connections. */
export function useDeletePluginConnection() {
  const queryClient = useQueryClient();
  return useMutation(UserService.method.deletePluginConnection, {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: connectionsKey });
    },
  });
}
