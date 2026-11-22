import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { ViewingEventService } from "@hub/gen/nama/hub/v1/viewing_pb";
import { useQueryClient } from "@tanstack/react-query";

const eventsKey = createConnectQueryKey({
  schema: ViewingEventService.method.listViewingEvents,
  cardinality: "finite",
});

/** Every one of the signed-in Account's Viewing Events, the latest started first. */
export function useViewingEvents() {
  return useQuery(ViewingEventService.method.listViewingEvents, {});
}

/** Deletes one of the signed-in Account's Viewing Events, then lists them again. */
export function useDeleteViewingEvent() {
  const queryClient = useQueryClient();
  return useMutation(ViewingEventService.method.deleteViewingEvent, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: eventsKey });
    },
  });
}
