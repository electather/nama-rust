import { useMutation } from "@connectrpc/connect-query";
import { refusalReason } from "@hub/features/account/lib/refusals";
import { AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { RefusalReason } from "@hub/gen/nama/hub/v1/refusal_pb";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

/** Creates an Account, which signs it in, then opens the Account page. */
export function useCreateAccount() {
  return useMutation(AccountService.method.createAccount, { onSuccess: useGoAfterSession("/") });
}

/** Signs an Account in, then goes on to the Hub path `continueTo`. */
export function useSignIn(continueTo: string) {
  return useMutation(AccountService.method.signIn, { onSuccess: useGoAfterSession(continueTo) });
}

/** Signs this browser out, then opens the sign-in page. */
export function useSignOut() {
  const goToSignIn = useGoAfterSession("/sign-in");
  return useMutation(AccountService.method.signOut, {
    onSuccess: goToSignIn,
    onError: async (error) => {
      // A session that ended while the page was open is signed out all the same.
      if (refusalReason(error) === RefusalReason.SIGN_IN_REQUIRED) {
        await goToSignIn();
      }
    },
  });
}

/** Deletes the signed-in Account, which signs this browser out, then opens the sign-in page. */
export function useDeleteAccount() {
  return useMutation(AccountService.method.deleteAccount, {
    onSuccess: useGoAfterSession("/sign-in"),
  });
}

/** Once the session changed: forgets what was cached for whoever was signed in, and goes `to`. */
export function useGoAfterSession(to: string) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  return async () => {
    queryClient.clear();
    await navigate({ href: to });
  };
}
