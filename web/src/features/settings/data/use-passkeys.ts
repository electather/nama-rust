import { createConnectQueryKey, useSuspenseQuery, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** The signed-in Account's passkeys, oldest first. */
export function usePasskeys() {
  return useSuspenseQuery(AccountService.method.listPasskeys, {}).data.passkeys;
}

/** Lists the passkeys again, after adding or removing one. */
export function useRefreshPasskeys() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return async () => {
    await queryClient.invalidateQueries({
      queryKey: createConnectQueryKey({
        schema: AccountService.method.listPasskeys,
        cardinality: "finite",
        transport,
      }),
    });
  };
}
