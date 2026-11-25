import { useMutation } from "@connectrpc/connect-query";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Makes this Device act as another of its Users, then calls `onSwitched`. */
export function useSwitchUser(onSwitched: () => Promise<void>) {
  return useMutation(SignInService.method.switchUser, { onSuccess: onSwitched });
}
