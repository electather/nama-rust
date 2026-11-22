import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { serverSettingsKey } from "@/features/settings/data/use-server-settings";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { invitesKey } from "@/shared/hooks/invites-key";

const serverInfoKey = createConnectQueryKey({
  schema: ServerService.method.getServerInfo,
  cardinality: "finite",
});

/** Saves the Server's settings, then refetches them, the Invites and the Server's info. */
export function useUpdateServerSettings() {
  const queryClient = useQueryClient();
  return useMutation(ServerService.method.updateServerSettings, {
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: serverSettingsKey }),
        // The Invites say whether one can be made, which takes a Server URL.
        queryClient.invalidateQueries({ queryKey: invitesKey }),
        // Whether it exports Diagnostics follows Share Diagnostics. Its query is `static`, which
        // `invalidateQueries` skips; other Devices' tabs see it only once reloaded.
        queryClient.resetQueries({ queryKey: serverInfoKey }),
      ]);
    },
  });
}
