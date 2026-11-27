import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";
import { signWithPasskey } from "@/shared/lib/webauthn";

/** Signs an Account in at the Hub with a passkey the browser offers, then goes on to `redirect`. */
export function useSignInWithPasskey(redirect: string | undefined) {
  const signIn = useIdentityChange({ kind: "account signed in", goOn: redirect });
  return useMutation({
    mutationFn: async () => {
      await signIn(async (hub) => {
        const started = await callUnaryMethod(hub, AccountService.method.startPasskeySignIn, {});
        const signed = await signWithPasskey(started);
        await callUnaryMethod(hub, AccountService.method.signInWithPasskey, signed);
      });
    },
  });
}
