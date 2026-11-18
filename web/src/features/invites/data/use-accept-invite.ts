import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Creates a User with an Invite's link, signs them in, and opens the home page. */
export function useAcceptInvite() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  return useMutation(SignInService.method.acceptInvite, {
    onSuccess: async () => {
      // Forget anything cached for whoever was signed in on this browser before.
      queryClient.clear();
      await navigate({ to: "/" });
    },
  });
}
