import type { MessageInitShape } from "@bufbuild/protobuf";
import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { type SignInRequestSchema, SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Signs in with a username and password, then goes on to `redirect`. */
export function useSignIn(redirect: string | undefined) {
  const signIn = useIdentityChange({ kind: "signed in", goOn: redirect });
  return useMutation({
    mutationFn: async (input: MessageInitShape<typeof SignInRequestSchema>) => {
      await signIn(async (server) => {
        await callUnaryMethod(server, SignInService.method.signIn, input);
      });
    },
  });
}
