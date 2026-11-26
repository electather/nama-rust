import { useQuery } from "@connectrpc/connect-query";

import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";
import { useMay } from "@/shared/hooks/use-may";

/**
 * The Show `showId` from the title source, with every Season and Episode and their Requests, for a
 * User who may Request Shows; nothing for one who may not, or while it loads or if it fails.
 */
export function useFoundShowIfMay(showId: ExternalId) {
  const enabled = useMay(Permission.REQUEST_TV);
  const options = { staleTime: loaderStaleTime, enabled };
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the missing Episodes are an extra on a Library's Show, which shows its own without them while they load or if the title source fails
  return useQuery(RequestService.method.lookUpShow, { showId }, options).data?.show;
}
