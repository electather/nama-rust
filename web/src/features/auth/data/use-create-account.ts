import type { MessageInitShape } from "@bufbuild/protobuf";
import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { AccountService, type CreateAccountRequestSchema } from "@/gen/nama/hub/v1/account_pb";

/** Creates an Account at the Hub, which signs it in, then goes on to `redirect`. */
export function useCreateAccount(redirect: string | undefined) {
  const signIn = useIdentityChange({ kind: "account signed in", goOn: redirect });
  return useMutation({
    mutationFn: async (input: MessageInitShape<typeof CreateAccountRequestSchema>) => {
      await signIn(async (hub) => {
        await callUnaryMethod(hub, AccountService.method.createAccount, input);
      });
    },
  });
}
