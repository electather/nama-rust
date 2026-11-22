import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { serverSettingsKey } from "@/features/settings/data/use-server-settings";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { serverInfoKey } from "@/shared/hooks/access";
import { invitesKey } from "@/shared/hooks/invites-key";

/** Saves the Server's settings, then refetches them, the Invites and the Server's info. */
export function useUpdateServerSettings() {
  const queryClient = useQueryClient();
  return useMutation(ServerService.method.updateServerSettings, {
    onSuccess: async () => {
      // The Invites say whether one can be made, which takes a Server URL.
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: serverSettingsKey }),
        queryClient.invalidateQueries({ queryKey: invitesKey }),
        // Whether the Server exports Diagnostics follows sharing them, so the next page starts or
        // stops recording; reset, as the static answer ignores invalidating.
        queryClient.resetQueries({ queryKey: serverInfoKey }),
      ]);
    },
  });
}
