import { createConnectQueryKey, useQuery } from "@connectrpc/connect-query";
import type { QueryClient } from "@tanstack/react-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { UserService } from "@/gen/nama/v1/user_pb";

/** What changing the signed-in User's Home layout changes: it, and their Home. */
const changedKeys = [
  createConnectQueryKey({ schema: UserService.method.getHomeLayout, cardinality: "finite" }),
  createConnectQueryKey({ schema: BrowseService.method.listHomeRows, cardinality: "finite" }),
  createConnectQueryKey({ schema: BrowseService.method.listFeatured, cardinality: "finite" }),
];

/** Refetches what changing the signed-in User's Home layout changes. */
export async function refetchHomeLayout(queryClient: QueryClient) {
  await Promise.all(
    changedKeys.map(async (queryKey) => {
      await queryClient.invalidateQueries({ queryKey });
    }),
  );
}

/** The signed-in User's Home layout: every Home row they could see, each shown or hidden. */
export function useHomeLayout() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the section shows nothing until it loads
  return useQuery(UserService.method.getHomeLayout, {});
}
