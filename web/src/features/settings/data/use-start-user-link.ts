import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { userLinkKey } from "@/features/settings/data/use-user-link";
import { UserService } from "@/gen/nama/v1/user_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/** Starts Linking the signed-in User to an Account, once they confirm it's them, then shows the code. */
export function useStartUserLink() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useSensitiveMutation(UserService.method.startUserLink, {
    confirming: "Link to your Account on the Hub",
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: userLinkKey(transport) });
    },
  });
}
