import { useMutation } from "@connectrpc/connect-query";

import { type StartSignInCodeResponse, SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Asks for a Sign-in Code for this browser, handing the Server's answer to `onStarted`. */
export function useStartSignInCode(onStarted: (shown: StartSignInCodeResponse) => void) {
  return useMutation(SignInService.method.startSignInCode, { onSuccess: onStarted });
}
