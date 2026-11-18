import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { serverSettingsKey } from "@/features/settings/data/use-server-settings";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { invitesKey } from "@/shared/hooks/invites-key";

/** Saves the Server's settings, then refetches them and the Invites. */
export function useUpdateServerSettings() {
  const queryClient = useQueryClient();
  return useMutation(ServerService.method.updateServerSettings, {
    onSuccess: async () => {
      // The Invites say whether one can be made, which takes a Server URL.
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: serverSettingsKey }),
        queryClient.invalidateQueries({ queryKey: invitesKey }),
      ]);
    },
  });
}
