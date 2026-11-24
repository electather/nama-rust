import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";

/** What saving the default Home layout changes: it, and the Admin's own Home, which follows it. */
function changedKeys(transport: Transport) {
  const finite = { transport, cardinality: "finite" } as const;
  return [
    createConnectQueryKey({ schema: ServerService.method.getDefaultHomeLayout, ...finite }),
    createConnectQueryKey({ schema: BrowseService.method.listHomeRows, ...finite }),
    createConnectQueryKey({ schema: BrowseService.method.listFeatured, ...finite }),
  ];
}

/** Saves the default Home layout, then refetches what it changes. */
export function useSetDefaultHomeLayout() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(ServerService.method.setDefaultHomeLayout, {
    // One at a time, so the newest whole layout is the one the Server keeps.
    scope: { id: "set-default-home-layout" },
    onSuccess: async () => {
      await Promise.all(
        changedKeys(transport).map(async (queryKey) => {
          await queryClient.invalidateQueries({ queryKey });
        }),
      );
    },
  });
}
