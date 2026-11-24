import { Code, type ConnectError } from "@connectrpc/connect";
import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { devicesKey } from "@/shared/hooks/devices-key";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";
import { isCode } from "@/shared/lib/connect-errors";

/**
 * Signs in the Device named `deviceName` from the Devices pane, once the User confirms it's them:
 * the Devices are fetched again so it's listed, then `onApproved`. A refusal goes to `onRefused`;
 * cancelling "Confirm it’s you" isn't one.
 */
export function useApproveDevice(
  deviceName: string,
  onApproved: () => void,
  onRefused: (error: ConnectError) => void,
) {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useSensitiveMutation(SignInService.method.approveSignInCode, {
    confirming: `sign in “${deviceName}”`,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: devicesKey(transport) });
      onApproved();
    },
    onError: (error) => {
      if (!isCode(error, Code.Canceled)) {
        onRefused(error);
      }
    },
  });
}
