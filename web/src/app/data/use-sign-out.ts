import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { UserService } from "@/gen/nama/v1/user_pb";
import { whoIsSignedIn } from "@/shared/hooks/access";

/**
 * Signs the User out of this Device, leaving its other Users signed in, and opens the User picker,
 * which goes on to sign-in once nobody is left (ADR 0106).
 */
export function useSignOut() {
  const queryClient = useQueryClient();
  const signOut = useIdentityChange({ kind: "user signed out" });
  return useMutation({
    mutationFn: async () => {
      await signOut(async (server) => {
        const signedIn = await whoIsSignedIn({ queryClient, transport: server });
        await callUnaryMethod(server, UserService.method.revokeDevice, {
          // Signed out meanwhile, no Device is named, and the Server refuses as it should.
          deviceId: signedIn?.device?.id ?? 0n,
        });
      });
    },
  });
}
