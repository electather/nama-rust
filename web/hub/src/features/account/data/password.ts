import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { useRefreshSessions } from "@hub/features/account/data/sessions";
import { AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { useQueryClient } from "@tanstack/react-query";

/** The signed-in Account's email and whether it has a password; `undefined` until the Hub has said. */
export function usePasswordStatus() {
  const { data } = useQuery(AccountService.method.getAccount, {});
  return data?.account && { email: data.account.email, hasPassword: data.hasPassword };
}

/**
 * Sets the signed-in Account's password, then asks again whether it has one, and where it's signed
 * in, as that signs it out everywhere else.
 */
export function useSetPassword() {
  const queryClient = useQueryClient();
  const refreshSessions = useRefreshSessions();
  return useMutation(AccountService.method.setPassword, {
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: createConnectQueryKey({
            schema: AccountService.method.getAccount,
            cardinality: "finite",
          }),
        }),
        refreshSessions(),
      ]);
    },
  });
}
