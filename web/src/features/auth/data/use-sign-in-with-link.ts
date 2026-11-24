import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate, useRouter } from "@tanstack/react-router";

import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";

/**
 * Uses a Sign-in Link up, signing its Account in, then goes where the link said to continue. A link
 * that expired meanwhile has its page look it up again, which then says so.
 */
export function useSignInWithLink() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  return useMutation(AccountService.method.signInWithLink, {
    onSuccess: async ({ continueTo }) => {
      queryClient.clear();
      // A used link can't sign in again, so Back skips its page (`src/routes/AGENTS.md`).
      await navigate({ href: continueTo, replace: true });
    },
    onError: async (error) => {
      if (hubRefusalReason(error) === RefusalReason.SIGN_IN_LINK_EXPIRED) {
        await router.invalidate();
      }
    },
  });
}
