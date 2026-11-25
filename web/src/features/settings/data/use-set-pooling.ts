import { createConnectQueryKey, useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { useSignInIfSessionEnded } from "@/features/auth/data/use-sign-in-if-session-ended";
import { ViewingEventService } from "@/gen/nama/hub/v1/viewing_pb";

/** Turns pooling on or off for the signed-in Account, then reads it again. */
export function useSetPooling() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(ViewingEventService.method.setPooling, {
    onError: useSignInIfSessionEnded(),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: createConnectQueryKey({
          schema: ViewingEventService.method.getPooling,
          cardinality: "finite",
          transport,
        }),
      });
    },
  });
}
