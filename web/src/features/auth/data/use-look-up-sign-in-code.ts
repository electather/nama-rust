import { useMutation } from "@connectrpc/connect-query";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Looks up the Device a Sign-in Code belongs to. */
export function useLookUpSignInCode() {
  return useMutation(SignInService.method.lookUpSignInCode);
}
