import { useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Permissions new Users get, which a new Invite starts from. */
export function useDefaultPermissions() {
  return useDefaults().permissions;
}

/** The cap for remote playback new Users get. */
export function useDefaultRemoteCap() {
  return useDefaults().remotePlaybackLimit;
}

function useDefaults() {
  return useSuspenseQuery(
    UserService.method.getDefaultPermissions,
    {},
    { staleTime: loaderStaleTime },
  ).data;
}
