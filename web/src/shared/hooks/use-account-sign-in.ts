import { useQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Whether this Server is Linked to the Hub, so its Users can sign in with their Accounts. */
export function useAccountSignIn() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the page shows at once, and offers signing in with an Account once the Server says it may
  return useQuery(ServerService.method.getServerInfo, {}, { staleTime: loaderStaleTime }).data
    ?.accountSignIn;
}
