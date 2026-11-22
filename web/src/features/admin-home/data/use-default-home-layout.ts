import { create } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { HomeLayoutSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The default Home layout, with every Home row Home could show now. */
export function useDefaultHomeLayout() {
  return (
    useSuspenseQuery(ServerService.method.getDefaultHomeLayout, {}, { staleTime: loaderStaleTime })
      .data.layout ?? create(HomeLayoutSchema)
  );
}
