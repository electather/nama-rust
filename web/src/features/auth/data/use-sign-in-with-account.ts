import type { MessageInitShape } from "@bufbuild/protobuf";
import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { type SignInWithAccountRequestSchema, SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Finishes signing in with an Account the Hub approved, then goes on to `redirect`. */
export function useSignInWithAccount(redirect: string | undefined) {
  const signIn = useIdentityChange({ kind: "signed in", goOn: redirect });
  return useMutation({
    mutationFn: async (input: MessageInitShape<typeof SignInWithAccountRequestSchema>) => {
      await signIn(async (server) => {
        await callUnaryMethod(server, SignInService.method.signInWithAccount, input);
      });
    },
  });
}
