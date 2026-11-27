import type { MessageInitShape } from "@bufbuild/protobuf";
import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { type AcceptInviteRequestSchema, SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Creates a User with an Invite's link, signs them in, and opens the home page. */
export function useAcceptInvite() {
  const join = useIdentityChange({ kind: "signed in", goOn: undefined });
  return useMutation({
    mutationFn: async (input: MessageInitShape<typeof AcceptInviteRequestSchema>) => {
      await join(async (server) => {
        await callUnaryMethod(server, SignInService.method.acceptInvite, input);
      });
    },
  });
}
