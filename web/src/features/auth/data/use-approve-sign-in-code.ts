import { Code, type ConnectError } from "@connectrpc/connect";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";
import { isCode } from "@/shared/lib/connect-errors";

/**
 * Signs in the Device named `deviceName` a Sign-in Code belongs to, once the User confirms it's
 * them, then calls `onApproved`. A refusal goes to `onRefused`; cancelling "Confirm it’s you"
 * isn't one.
 */
export function useApproveSignInCode(
  deviceName: string,
  onApproved: () => Promise<void>,
  onRefused: (error: ConnectError) => Promise<void>,
) {
  return useSensitiveMutation(SignInService.method.approveSignInCode, {
    confirming: `sign in “${deviceName}”`,
    onSuccess: onApproved,
    onError: async (error) => {
      if (!isCode(error, Code.Canceled)) {
        await onRefused(error);
      }
    },
  });
}
