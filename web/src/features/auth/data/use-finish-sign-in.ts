import { useTransport } from "@connectrpc/connect-query";

import { useFinishHubSignIn } from "@/features/auth/data/use-finish-hub-sign-in";
import { moveBrowserPush } from "@/shared/hooks/browser-push";

/**
 * Once someone is signed in to a Server, or switched to: hands them this browser's notifications,
 * then finishes as `useFinishHubSignIn` does.
 */
export function useFinishSignIn(redirect: string | undefined) {
  const transport = useTransport();
  const finish = useFinishHubSignIn(redirect);
  return async () => {
    // The Device now acts as this User (ADR 0106), and the Server keeps pushing whoever it was.
    await moveBrowserPush(transport);
    await finish();
  };
}
