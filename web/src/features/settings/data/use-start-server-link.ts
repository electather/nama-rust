import { useQueryClient } from "@tanstack/react-query";

import { serverLinkKey } from "@/features/settings/data/use-server-link";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/** Starts Linking the Server, once the User confirms it's them, then shows the code. */
export function useStartServerLink() {
  const queryClient = useQueryClient();
  return useSensitiveMutation(ServerService.method.startServerLink, {
    confirming: "Link the Server to the Hub",
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: serverLinkKey });
    },
  });
}
