import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { playsKey } from "@/shared/hooks/plays-key";

/** Stops a play on any Device, then refetches the plays, whether or not its event arrives first. */
export function useStopPlay() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(ServerService.method.stopPlay, {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: playsKey(transport) });
    },
  });
}
