import { useMutation } from "@connectrpc/connect-query";
import { useNavigate } from "@tanstack/react-router";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

// The Server's lifetime for a Sign-in Code (`sign_in.proto`).
const lifetime = 10 * 60 * 1000;

/**
 * Asks for a Sign-in Code for this browser, then shows it at `/sign-in/code/waiting`, in place of
 * this page when `replace`, such as for a new code in place of an expired one.
 */
export function useStartSignInCode(redirect: string | undefined, replace: boolean) {
  const navigate = useNavigate();
  return useMutation(SignInService.method.startSignInCode, {
    onSuccess: async ({ code, handle }, { deviceName = "" }) => {
      // ponytail: the Server doesn't send when it expires, so this counts its lifetime from now.
      const expiresAt = Date.now() + lifetime;
      await navigate({
        to: "/sign-in/code/waiting",
        search: { redirect },
        state: { waitingCode: { code, handle, deviceName, expiresAt } },
        replace,
      });
    },
  });
}
