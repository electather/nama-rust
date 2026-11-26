import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { forgetServerSessions } from "@/features/servers/lib/server-sessions";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";
import { signWithPasskey } from "@/shared/lib/webauthn";

/** Signs an Account in at the Hub with a passkey the browser offers, then calls `onSignedIn`. */
export function useSignInWithPasskey(onSignedIn: () => Promise<void>) {
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
    onSuccess: async () => {
      // A Device kept from before lacks this sign-in's Recent Authentication; a new trade has it.
      forgetServerSessions();
      await onSignedIn();
    },
  });
}
