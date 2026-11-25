import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { UserService } from "@/gen/nama/v1/user_pb";
import { whoIsSignedIn } from "@/shared/hooks/access";

/**
 * Signs the User out of this Device, leaving its other Users signed in, forgets everything cached
 * for them, and opens the User picker, which goes on to sign-in once nobody is left (ADR 0106).
 */
export function useSignOut() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const navigate = useNavigate();
  return useMutation({
    mutationFn: async () => {
      const signedIn = await whoIsSignedIn({ queryClient, transport });
      await callUnaryMethod(transport, UserService.method.revokeDevice, {
        // Signed out meanwhile, no Device is named, and the Server refuses as it should.
        deviceId: signedIn?.device?.id ?? 0n,
      });
    },
    onSuccess: async () => {
      // Nothing cached for the User who just left may be shown to whoever signs in next.
      queryClient.clear();
      await navigate({ to: "/switch-user" });
    },
  });
}
