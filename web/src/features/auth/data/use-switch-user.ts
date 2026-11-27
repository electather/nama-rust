import type { MessageInitShape } from "@bufbuild/protobuf";
import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { type SwitchUserRequestSchema, SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Makes this Device act as another of its Users, then goes on to `redirect`. */
export function useSwitchUser(redirect: string | undefined) {
  const switchTo = useIdentityChange({ kind: "switched", goOn: redirect });
  return useMutation({
    mutationFn: async (input: MessageInitShape<typeof SwitchUserRequestSchema>) => {
      await switchTo(async (server) => {
        await callUnaryMethod(server, SignInService.method.switchUser, input);
      });
    },
  });
}
