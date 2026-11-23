import type { Transport } from "@connectrpc/connect";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import type { RouterContext } from "@hub/app/router-context";
import { refusalReason } from "@hub/features/account/lib/refusals";
import { AccountService, type GetAccountResponse } from "@hub/gen/nama/hub/v1/account_pb";
import { RefusalReason } from "@hub/gen/nama/hub/v1/refusal_pb";
import { queryOptions, useQuery, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";

/**
 * What the Hub says of the signed-in Account, or `null` when nobody is signed in: being signed out
 * isn't an error. Every reader shares this one answer, so a page asks for it once. It stays cached
 * until a sign-in, a sign-out or a change to the Account refreshes it.
 */
function accountQuery(transport: Transport) {
  return queryOptions({
    queryKey: ["account"],
    queryFn: async () => {
      try {
        return await callUnaryMethod(transport, AccountService.method.getAccount, {});
      } catch (error) {
        if (refusalReason(error) === RefusalReason.SIGN_IN_REQUIRED) {
          return null;
        }
        throw error;
      }
    },
    staleTime: "static",
  });
}

function accountOf(answer: GetAccountResponse | null) {
  return answer?.account ?? null;
}

/** The signed-in Account, for a route's access check. */
export async function signedInAccount({ queryClient, transport }: RouterContext) {
  const answer = await queryClient.query(accountQuery(transport));
  return accountOf(answer);
}

/** The signed-in Account, which the route's access check has already read. */
export function useAccount() {
  const { data } = useSuspenseQuery({ ...accountQuery(useTransport()), select: accountOf });
  return data;
}

/**
 * The signed-in Account's query, for a page no access check guards: it doesn't suspend, and an
 * error stays in the result instead of throwing, so the page still renders when the Hub is down.
 */
export function useAccountQuery() {
  return useQuery({ ...accountQuery(useTransport()), select: accountOf });
}

/** The signed-in Account's email and whether it has a password; `undefined` until the Hub has said. */
export function usePasswordStatus() {
  const { data } = useQuery(accountQuery(useTransport()));
  return data?.account && { email: data.account.email, hasPassword: data.hasPassword };
}

/**
 * Asks the Hub for the signed-in Account again, after a change to it. If that fails, readers keep
 * what the Hub said before.
 */
export function useRefreshAccount() {
  const queryClient = useQueryClient();
  const options = accountQuery(useTransport());
  return async () => {
    try {
      // `invalidateQueries` never refetches a static query, so this fetches past `staleTime`.
      await queryClient.query({ ...options, staleTime: 0 });
    } catch {
      // The change itself succeeded; failing to show it isn't the caller's error.
    }
  };
}

/**
 * Forgets whoever was signed in before a sign-in just now, so the next route's access check asks
 * the Hub for the Account, and whether it has a password, which the sign-in's answer doesn't say.
 */
export function useRecordSignedIn() {
  const queryClient = useQueryClient();
  const { queryKey } = accountQuery(useTransport());
  return () => {
    // Not `resetQueries`: the page signing in still reads the Account, and would suspend.
    queryClient.removeQueries({ queryKey });
  };
}
