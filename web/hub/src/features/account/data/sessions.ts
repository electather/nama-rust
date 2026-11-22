import { useMutation } from "@connectrpc/connect-query";
import { AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

/** Creates an Account, which signs it in, then opens the Account page. */
export function useCreateAccount() {
  return useMutation(AccountService.method.createAccount, { onSuccess: useGoAfterSession("/") });
}

/** Signs an Account in, then opens the Account page. */
export function useSignIn() {
  return useMutation(AccountService.method.signIn, { onSuccess: useGoAfterSession("/") });
}

/** Signs this browser out, then opens the sign-in page. */
export function useSignOut() {
  return useMutation(AccountService.method.signOut, {
    onSuccess: useGoAfterSession("/sign-in"),
  });
}

/** Once the session changed: forgets what was cached for whoever was signed in, and goes `to`. */
function useGoAfterSession(to: "/" | "/sign-in") {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  return async () => {
    queryClient.clear();
    await navigate({ to });
  };
}
