import { createConnectQueryKey, useSuspenseQuery, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Where the signed-in Account is signed in: this browser first, then the most recently used. */
export function useAccountSessions() {
  return useSuspenseQuery(AccountService.method.listSessions, {}).data.sessions;
}

/**
 * Lists the sessions again, after anything that may end some, refused or not: one another tab
 * signed out is gone all the same.
 */
export function useRefreshAccountSessions() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return async () => {
    await queryClient.invalidateQueries({
      queryKey: createConnectQueryKey({
        schema: AccountService.method.listSessions,
        cardinality: "finite",
        transport,
      }),
    });
  };
}
