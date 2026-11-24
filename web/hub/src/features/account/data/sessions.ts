import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
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
  return useMutation(AccountService.method.signOut, {
    onSuccess: useGoAfterSession("/sign-in"),
    onError: useSignInIfSessionEnded(),
  });
}

/**
 * An `onError` that opens the sign-in page when the call was refused because this browser's
 * session already ended, elsewhere or by expiring: trying again could never succeed.
 */
export function useSignInIfSessionEnded() {
  const goToSignIn = useGoAfterSession("/sign-in");
  return async (error: Error) => {
    if (refusalReason(error) === RefusalReason.SIGN_IN_REQUIRED) {
      await goToSignIn();
    }
  };
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

const sessionsKey = createConnectQueryKey({
  schema: AccountService.method.listSessions,
  cardinality: "finite",
});

/**
 * Where the signed-in Account is signed in: this browser first, then the most recently used. It
 * doesn't suspend, as `usePasskeys` doesn't: the rest of the Account page works without the list.
 */
export function useSessions() {
  return useQuery(AccountService.method.listSessions, {});
}

/** Signs out one of the Account's other sessions, by its id. */
export function useSignOutSession() {
  return useMutation(AccountService.method.signOutSession, {
    onError: useSignInIfSessionEnded(),
    onSettled: useRefreshSessions(),
  });
}

/** Signs out every session of the Account but this browser's. */
export function useSignOutOtherSessions() {
  return useMutation(AccountService.method.signOutOtherSessions, {
    onError: useSignInIfSessionEnded(),
    onSettled: useRefreshSessions(),
  });
}

/**
 * Lists the sessions again, after anything that may end some: one another tab signed out is gone
 * all the same.
 */
export function useRefreshSessions() {
  const queryClient = useQueryClient();
  return async () => {
    await queryClient.invalidateQueries({ queryKey: sessionsKey });
  };
}
