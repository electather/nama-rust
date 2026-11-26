import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useSignInIfSessionEnded } from "@/features/auth/data/use-sign-in-if-session-ended";
import { createPasskey } from "@/features/auth/lib/webauthn";
import { useRefreshPasskeys } from "@/features/settings/data/use-passkeys";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Has the browser create a passkey for the Account with `email`, then adds it as `name`. */
export function useAddPasskey() {
  const transport = useTransport();
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
    onSuccess: useRefreshPasskeys(),
    onError: useSignInIfSessionEnded(),
  });
}
