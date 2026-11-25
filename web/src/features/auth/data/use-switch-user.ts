import { Code } from "@connectrpc/connect";
import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { signedInQuery } from "@/shared/hooks/access";
import { isCode } from "@/shared/lib/connect-errors";

/** Makes this Device act as another of its Users, then calls `onSwitched`. */
export function useSwitchUser(onSwitched: () => Promise<void>) {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const router = useRouter();
  return useMutation(SignInService.method.switchUser, {
    onSuccess: onSwitched,
    onError: async (error) => {
      // Signed out of this Device elsewhere: asking again who is lets the picker drop them.
      if (isCode(error, Code.NotFound)) {
        queryClient.removeQueries({ queryKey: signedInQuery(transport).queryKey });
        await router.invalidate();
      }
    },
  });
}
