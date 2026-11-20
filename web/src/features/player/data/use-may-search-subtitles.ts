import { useSuspenseQuery } from "@connectrpc/connect-query";
import { useRouteContext } from "@tanstack/react-router";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";
import { may } from "@/shared/lib/permissions";

/** Whether the User may search for subtitles: they have Download subtitles, and a source is on. */
export function useMaySearchSubtitles() {
  const { signedIn } = useRouteContext({ from: "/_member" });
  // Every signed-in page has the Server's info already: its access check fetched it. Past
  // `loaderStaleTime` the player asks again, as an Admin may have enabled a source since.
  const { searchesSubtitles } = useSuspenseQuery(
    ServerService.method.getServerInfo,
    {},
    { staleTime: loaderStaleTime },
  ).data;
  return searchesSubtitles && may(signedIn.user, Permission.DOWNLOAD_SUBTITLES);
}
