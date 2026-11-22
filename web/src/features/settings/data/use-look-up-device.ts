import { useMutation } from "@connectrpc/connect-query";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Looks up which Device a Sign-in Code belongs to, from the Devices pane. */
export function useLookUpDevice() {
  return useMutation(SignInService.method.lookUpSignInCode);
}
