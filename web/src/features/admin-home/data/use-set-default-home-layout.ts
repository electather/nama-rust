import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";

/** What saving the default Home layout changes: it, and the Admin's own Home, which follows it. */
const changedKeys = [
  createConnectQueryKey({
    schema: ServerService.method.getDefaultHomeLayout,
    cardinality: "finite",
  }),
  createConnectQueryKey({ schema: BrowseService.method.listHomeRows, cardinality: "finite" }),
  createConnectQueryKey({ schema: BrowseService.method.listFeatured, cardinality: "finite" }),
];

/** Saves the default Home layout, then refetches what it changes. */
export function useSetDefaultHomeLayout() {
  const queryClient = useQueryClient();
  return useMutation(ServerService.method.setDefaultHomeLayout, {
    onSuccess: async () => {
      await Promise.all(
        changedKeys.map(async (queryKey) => {
          await queryClient.invalidateQueries({ queryKey });
        }),
      );
    },
  });
}
