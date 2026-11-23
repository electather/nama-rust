import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { ViewingEventService } from "@hub/gen/nama/hub/v1/viewing_pb";
import { useQueryClient } from "@tanstack/react-query";

const poolingKey = createConnectQueryKey({
  schema: ViewingEventService.method.getPooling,
  cardinality: "finite",
});

/**
 * Whether the signed-in Account pools the Movies it finishes, on until it's read otherwise, and
 * turning it on or off. It doesn't suspend: the pages it shows on work without it.
 */
export function usePooling() {
  const queryClient = useQueryClient();
  const pooling = useQuery(ViewingEventService.method.getPooling, {});
  const setPooling = useMutation(ViewingEventService.method.setPooling, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: poolingKey });
    },
  });
  return {
    on: pooling.data?.pooling !== false,
    busy: pooling.data === undefined || setPooling.isPending,
    failed: pooling.isError || setPooling.isError,
    set: (on: boolean) => {
      setPooling.mutate({ pooling: on });
    },
  };
}
