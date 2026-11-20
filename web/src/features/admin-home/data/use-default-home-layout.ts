import { create } from "@bufbuild/protobuf";
import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { HomeLayoutSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The default Home layout, for the Home page to load before it shows the list. */
export function defaultHomeLayoutQuery(transport: Transport) {
  return {
    ...createQueryOptions(ServerService.method.getDefaultHomeLayout, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The default Home layout, with every Home row Home could show now. */
export function useDefaultHomeLayout() {
  return (
    useSuspenseQuery(ServerService.method.getDefaultHomeLayout, {}, { staleTime: loaderStaleTime })
      .data.layout ?? create(HomeLayoutSchema)
  );
}
