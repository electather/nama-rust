import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { serverSettingsKey } from "@/features/settings/data/use-server-settings";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { invitesKey } from "@/shared/hooks/invites-key";

function serverInfoKey(transport: Transport) {
  return createConnectQueryKey({
    schema: ServerService.method.getServerInfo,
    cardinality: "finite",
    transport,
  });
}

/** Saves the Server's settings, then refetches them and the Invites and resets the Server's info. */
export function useUpdateServerSettings() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(ServerService.method.updateServerSettings, {
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: serverSettingsKey(transport) }),
        // The Invites say whether one can be made, which takes a Server URL.
        queryClient.invalidateQueries({ queryKey: invitesKey(transport) }),
        // Share Diagnostics can change whether the Server exports Diagnostics. The info's query is
        // `static`, which `invalidateQueries` skips; other tabs still need a reload.
        queryClient.resetQueries({ queryKey: serverInfoKey(transport) }),
      ]);
    },
  });
}
