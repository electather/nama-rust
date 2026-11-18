import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Signs out, forgets everything cached for that User, and opens sign-in. */
export function useSignOut() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  return useMutation(SignInService.method.signOut, {
    onSuccess: async () => {
      // Nothing cached for the User who just left may be shown to whoever signs in next.
      queryClient.clear();
      await navigate({ to: "/sign-in" });
    },
  });
}
