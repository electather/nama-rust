import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate, useRouteContext } from "@tanstack/react-router";

import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import { useForgetServerSessions } from "@/features/servers/data/use-forget-server-sessions";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";

/**
 * Signs the Account out of this browser, forgets everything cached for it and its sessions on its
 * Servers, and opens sign-in, as it does when the session already ended, elsewhere or by expiring.
 * Signing in goes on to `redirect`.
 */
export function useSignOutAccount(redirect?: string) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const forgetServerSessions = useForgetServerSessions();
  // The Hub's, even from a Server's pages at the Hub, which run against that Server.
  const hub = useRouteContext({ from: "__root__", select: ({ transport }) => transport });
  // Only once its pages are gone, so none signs in to a Server again.
  const goToSignIn = async () => {
    await forgetServerSessions(async () => {
      queryClient.clear();
      await navigate({ to: "/sign-in", search: { redirect } });
    });
  };
  return useMutation(AccountService.method.signOut, {
    transport: hub,
    onSuccess: goToSignIn,
    onError: async (error) => {
      if (hubRefusalReason(error) === RefusalReason.SIGN_IN_REQUIRED) {
        await goToSignIn();
      }
    },
  });
}
