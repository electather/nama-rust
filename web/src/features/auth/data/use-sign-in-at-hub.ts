import type { MessageInitShape } from "@bufbuild/protobuf";
import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { AccountService, type SignInRequestSchema } from "@/gen/nama/hub/v1/account_pb";

/** Signs an Account in at the Hub with its email and password, then goes on to `redirect`. */
export function useSignInAtHub(redirect: string | undefined) {
  const signIn = useIdentityChange({ kind: "account signed in", goOn: redirect });
  return useMutation({
    mutationFn: async (input: MessageInitShape<typeof SignInRequestSchema>) => {
      await signIn(async (hub) => {
        await callUnaryMethod(hub, AccountService.method.signIn, input);
      });
    },
  });
}
