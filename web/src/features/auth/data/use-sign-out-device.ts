import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { leaveBrowserPush } from "@/shared/hooks/browser-push";

/**
 * Signs this Device out, with every User signed in to it, stops this browser's notifications,
 * forgets what was cached, and opens sign-in.
 */
export function useSignOutDevice() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const navigate = useNavigate();
  return useMutation(SignInService.method.signOut, {
    onMutate: async () => {
      await leaveBrowserPush(transport);
    },
    onSuccess: async () => {
      queryClient.clear();
      await navigate({ to: "/sign-in" });
    },
  });
}
