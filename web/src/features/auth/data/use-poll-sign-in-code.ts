import type { MessageInitShape } from "@bufbuild/protobuf";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import {
  type PollSignInCodeRequestSchema,
  SignInCodeStatus,
  SignInService,
} from "@/gen/nama/v1/sign_in_pb";

/**
 * Polls the sign-in a `handle` stands for, going on to `redirect` once it's approved.
 *
 * A poll is a mutation, not a query: a signed-in answer hands out the session only once.
 */
export function usePollSignInCode(redirect: string | undefined) {
  const transport = useTransport();
  const signIn = useIdentityChange({ kind: "signed in", goOn: redirect });
  return useMutation({
    mutationFn: async (input: MessageInitShape<typeof PollSignInCodeRequestSchema>) => {
      const answer = await callUnaryMethod(transport, SignInService.method.pollSignInCode, input);
      if (answer.status === SignInCodeStatus.SIGNED_IN) {
        // The poll that saw the approval holds the session: it was the call.
        await signIn(() => answer);
      }
      return answer;
    },
  });
}
