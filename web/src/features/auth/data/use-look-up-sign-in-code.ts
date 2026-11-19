import { useMutation } from "@connectrpc/connect-query";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Looks up the Device a Sign-in Code belongs to, handing both to `onFound`. */
export function useLookUpSignInCode(onFound: (code: string, deviceName: string) => Promise<void>) {
  return useMutation(SignInService.method.lookUpSignInCode, {
    onSuccess: async ({ deviceName }, { code = "" }) => {
      await onFound(code, deviceName);
    },
  });
}
