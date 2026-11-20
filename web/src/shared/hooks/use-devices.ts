import { Code, type Transport } from "@connectrpc/connect";
import { createQueryOptions, useQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";
import { isCode } from "@/shared/lib/connect-errors";
import type { Whose } from "@/shared/lib/devices-of";

/** The query for your own Devices, for a page's loader to fetch before it shows. */
export function yourDevicesQuery(transport: Transport) {
  return {
    ...createQueryOptions(UserService.method.listDevices, {}, { transport }),
    staleTime: "static" as const,
  };
}

/**
 * The Devices of `whose`. A failure shows the page's error, except that, for another User's, they
 * may have been deleted meanwhile.
 */
export function useDevices(whose: Whose, otherUser: boolean) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the list shows "Loading…" itself, and a deleted User's says so in place of the page's error
  return useQuery(UserService.method.listDevices, whose, {
    staleTime: loaderStaleTime,
    throwOnError: (failure) => !(otherUser && isCode(failure, Code.NotFound)),
  });
}
