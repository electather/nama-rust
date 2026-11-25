import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { serverLinkKey } from "@/features/settings/data/use-server-link";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/**
 * Starts Linking the Server, once the User confirms it's them: given a `returnUrl`, it opens the
 * Hub's approval, which comes back there, and otherwise it shows the code.
 */
export function useStartServerLink() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useSensitiveMutation(ServerService.method.startServerLink, {
    confirming: "Link the Server to the Hub",
    onSuccess: async ({ link }, { returnUrl = "" }) => {
      if (returnUrl !== "" && link) {
        globalThis.location.assign(link.url);
      }
      await queryClient.invalidateQueries({ queryKey: serverLinkKey(transport) });
    },
  });
}
