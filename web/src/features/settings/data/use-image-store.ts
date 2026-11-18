import { useSuspenseQuery } from "@connectrpc/connect-query";

import { isRunning } from "@/features/settings/lib/image-store";
import { ServerService } from "@/gen/nama/v1/server_pb";

/** How often to look again while the images are copied to another store. */
const SWITCHING_POLL_MS = 1000;

/** Where the images are kept and the copy to another store, looked at again while it runs. */
export function useImageStore() {
  return useSuspenseQuery(
    ServerService.method.getImageSettings,
    {},
    {
      refetchInterval: (query) =>
        isRunning(query.state.data?.switching) ? SWITCHING_POLL_MS : false,
    },
  ).data;
}
