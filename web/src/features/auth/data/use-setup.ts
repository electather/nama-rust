import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Creates the first Admin with the Setup Code, then opens the home page. */
export function useSetup() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  return useMutation(SignInService.method.setup, {
    onSuccess: async () => {
      // The cached Server info still says setup is needed, and nobody is signed in.
      queryClient.clear();
      await navigate({ to: "/" });
    },
  });
}
