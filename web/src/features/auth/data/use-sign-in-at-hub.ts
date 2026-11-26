import { useMutation } from "@connectrpc/connect-query";

import { forgetServerSessions } from "@/features/servers/lib/server-sessions";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Signs an Account in at the Hub with its email and password, then calls `onSignedIn`. */
export function useSignInAtHub(onSignedIn: () => Promise<void>) {
  return useMutation(AccountService.method.signIn, {
    onSuccess: async () => {
      // A Device kept from before lacks this sign-in's Recent Authentication; a new trade has it.
      forgetServerSessions();
      await onSignedIn();
    },
  });
}
