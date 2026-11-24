import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { connectionsKey } from "@/features/settings/data/use-plugin-connections";
import { UserService } from "@/gen/nama/v1/user_pb";

/** Starts a connection to a Plugin, opens its link, then refetches the connections. */
export function useStartPluginConnection() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(UserService.method.startPluginConnection, {
    // Opened once the link is made, so a browser may block it; the link stays on the page.
    onSuccess: ({ url }) => window.open(url, "_blank", "noopener"),
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: connectionsKey(transport) });
    },
  });
}
