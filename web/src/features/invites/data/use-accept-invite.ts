import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { moveBrowserPush } from "@/shared/hooks/browser-push";

/**
 * Creates a User with an Invite's link, signs them in, hands them this browser's notifications,
 * and opens the home page.
 */
export function useAcceptInvite() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(SignInService.method.acceptInvite, {
    onSuccess: async () => {
      // A browser signed in as someone else can open an Invite, and the Server keeps pushing them.
      await moveBrowserPush(transport);
      // Forget anything cached for whoever was signed in on this browser before.
      queryClient.clear();
      await navigate({ to: "/" });
    },
  });
}
