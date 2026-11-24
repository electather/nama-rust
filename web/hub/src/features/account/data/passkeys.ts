import {
  callUnaryMethod,
  createConnectQueryKey,
  useQuery,
  useTransport,
} from "@connectrpc/connect-query";
import { useGoAfterSession } from "@hub/features/account/data/sessions";
import {
  browserHasPasskeys,
  createPasskey,
  signWithPasskey,
} from "@hub/features/account/lib/webauthn";
import { AccountService } from "@hub/gen/nama/hub/v1/account_pb";
import { HubService } from "@hub/gen/nama/hub/v1/hub_pb";
import { useMutation, useQueryClient } from "@tanstack/react-query";

const passkeysKey = createConnectQueryKey({
  schema: AccountService.method.listPasskeys,
  cardinality: "finite",
});

/**
 * Whether passkeys work here: the Hub offers them, which it does once it has a public URL, and the
 * browser has them. Unknown is no, since the Hub refuses passkeys without that URL.
 */
export function usePasskeysWork() {
  const { data } = useQuery(HubService.method.getHubInfo, {});
  return data?.passkeys === true && browserHasPasskeys();
}

/**
 * The signed-in Account's passkeys, oldest first. It doesn't suspend, as `useLinkedServers` doesn't:
 * the rest of the Account page works without the list.
 */
export function usePasskeys() {
  return useQuery(AccountService.method.listPasskeys, {});
}

/** Has the browser create a passkey, then adds it to the Account with the name given. */
export function useAddPasskey() {
  const transport = useTransport();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ email, name }: { email: string; name: string }) => {
      const started = await callUnaryMethod(
        transport,
        AccountService.method.startAddingPasskey,
        {},
      );
      const created = await createPasskey(started, email, name);
      await callUnaryMethod(transport, AccountService.method.addPasskey, created);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: passkeysKey });
    },
  });
}

/** Removes one of the Account's passkeys by its id. */
export function useRemovePasskey() {
  const transport = useTransport();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (passkeyId: string) => {
      await callUnaryMethod(transport, AccountService.method.removePasskey, { passkeyId });
    },
    // A passkey another tab removed is gone all the same.
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: passkeysKey });
    },
  });
}

/** Signs in with a passkey the browser offers, then goes on to the Hub path `continueTo`. */
export function useSignInWithPasskey(continueTo: string) {
  const transport = useTransport();
  return useMutation({
    mutationFn: async () => {
      const started = await callUnaryMethod(
        transport,
        AccountService.method.startPasskeySignIn,
        {},
      );
      const signed = await signWithPasskey(started);
      await callUnaryMethod(transport, AccountService.method.signInWithPasskey, signed);
    },
    onSuccess: useGoAfterSession(continueTo),
  });
}
