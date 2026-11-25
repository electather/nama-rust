import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Signs this Device out, with every User signed in to it, forgets what was cached, and opens sign-in. */
export function useSignOutDevice() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  return useMutation(SignInService.method.signOut, {
    onSuccess: async () => {
      queryClient.clear();
      await navigate({ to: "/sign-in" });
    },
  });
}
