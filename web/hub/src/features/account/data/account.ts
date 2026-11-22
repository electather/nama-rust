import type { Transport } from "@connectrpc/connect";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import type { RouterContext } from "@hub/app/router-context";
import { refusalReason } from "@hub/features/account/lib/refusals";
import { AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { RefusalReason } from "@hub/gen/nama/hub/v1/refusal_pb";
import { queryOptions, useSuspenseQuery } from "@tanstack/react-query";

/**
 * The signed-in Account, or `null` when nobody is: being signed out isn't an error. It stays
 * cached until signing in or out clears the cache.
 */
function accountQuery(transport: Transport) {
  return queryOptions({
    queryKey: ["account"],
    queryFn: async () => {
      try {
        const answer = await callUnaryMethod(transport, AccountService.method.getAccount, {});
        return answer.account ?? null;
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

/** The signed-in Account, for a route's access check. */
export async function signedInAccount({ queryClient, transport }: RouterContext) {
  const account = await queryClient.query(accountQuery(transport));
  return account;
}

/** The signed-in Account, which the route's access check has already read. */
export function useAccount() {
  const { data } = useSuspenseQuery(accountQuery(useTransport()));
  return data;
}
