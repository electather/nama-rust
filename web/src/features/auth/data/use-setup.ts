import type { MessageInitShape } from "@bufbuild/protobuf";
import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { type SetupRequestSchema, SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Creates the first Admin with the Setup Code, which signs them in, then opens the home page. */
export function useSetup() {
  const setUp = useIdentityChange({ kind: "signed in", goOn: undefined });
  return useMutation({
    mutationFn: async (input: MessageInitShape<typeof SetupRequestSchema>) => {
      await setUp(async (server) => {
        await callUnaryMethod(server, SignInService.method.setup, input);
      });
    },
  });
}
