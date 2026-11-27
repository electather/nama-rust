import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Signs this Device out, with every User signed in to it, and opens sign-in. */
export function useSignOutDevice() {
  const signOut = useIdentityChange({ kind: "device signed out" });
  return useMutation({
    mutationFn: async () => {
      await signOut(async (server) => {
        await callUnaryMethod(server, SignInService.method.signOut, {});
      });
    },
  });
}
