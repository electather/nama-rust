import { useQueryClient } from "@tanstack/react-query";

import { serverLinkKey } from "@/features/settings/data/use-server-link";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/** Unlinks the Server, or stops showing its code, once the User confirms it's them. */
export function useUnlinkServer() {
  const queryClient = useQueryClient();
  return useSensitiveMutation(ServerService.method.unlinkServer, {
    confirming: "Unlink the Server",
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: serverLinkKey });
    },
  });
}
