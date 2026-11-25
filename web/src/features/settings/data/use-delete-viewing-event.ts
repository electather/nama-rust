import { createConnectQueryKey, useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { useSignInIfSessionEnded } from "@/features/auth/data/use-sign-in-if-session-ended";
import { ViewingEventService } from "@/gen/nama/hub/v1/viewing_pb";

/** Deletes one of the signed-in Account's Viewing Events, then lists them again. */
export function useDeleteViewingEvent() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(ViewingEventService.method.deleteViewingEvent, {
    onError: useSignInIfSessionEnded(),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: createConnectQueryKey({
          schema: ViewingEventService.method.listViewingEvents,
          cardinality: "finite",
          transport,
        }),
      });
    },
  });
}
