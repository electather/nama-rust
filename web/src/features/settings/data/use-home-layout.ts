import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";
import type { QueryClient } from "@tanstack/react-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { UserService } from "@/gen/nama/v1/user_pb";

/** What changing the signed-in User's Home layout changes: it, and their Home. */
function changedKeys(transport: Transport) {
  const finite = { transport, cardinality: "finite" } as const;
  return [
    createConnectQueryKey({ schema: UserService.method.getHomeLayout, ...finite }),
    createConnectQueryKey({ schema: BrowseService.method.listHomeRows, ...finite }),
    createConnectQueryKey({ schema: BrowseService.method.listFeatured, ...finite }),
  ];
}

/** Refetches what changing the signed-in User's Home layout on the Server `transport` talks to changes. */
export async function refetchHomeLayout(queryClient: QueryClient, transport: Transport) {
  await Promise.all(
    changedKeys(transport).map(async (queryKey) => {
      await queryClient.invalidateQueries({ queryKey });
    }),
  );
}

/** The signed-in User's Home layout: every Home row they could see, each shown or hidden. */
export function useHomeLayout() {
  return useSuspenseQuery(UserService.method.getHomeLayout, {}).data.layout;
}
