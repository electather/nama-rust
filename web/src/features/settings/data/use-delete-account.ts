import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { useSignInIfSessionEnded } from "@/features/auth/data/use-sign-in-if-session-ended";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Deletes the signed-in Account, which signs this browser out, then opens sign-in. */
export function useDeleteAccount() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  return useMutation(AccountService.method.deleteAccount, {
    onSuccess: async () => {
      queryClient.clear();
      await navigate({ to: "/sign-in" });
    },
    onError: useSignInIfSessionEnded(),
  });
}
