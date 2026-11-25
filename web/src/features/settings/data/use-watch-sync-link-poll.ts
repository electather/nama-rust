import { create } from "@bufbuild/protobuf";
import { useQuery, useTransport } from "@connectrpc/connect-query";
import { skipToken, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { personalKey } from "@/features/settings/data/use-personal-settings";
import { pollEveryMs } from "@/features/settings/lib/watch-sync-link";
import {
  PollWatchSyncLinkResponseSchema,
  type StartWatchSyncLinkResponse,
  UserService,
  WatchSyncLinkState,
} from "@/gen/nama/v1/user_pb";

/**
 * Whether the signed-in User entered the code `started` gave for the Watch sync Plugin
 * `pluginId`, asked as often as the Plugin allows while they haven't, until a poll fails; nothing
 * asked before they start. Once linked, the Personal settings are fetched again, as the Server now holds the tokens.
 */
export function useWatchSyncLinkPoll(
  pluginId: string,
  started: StartWatchSyncLinkResponse | undefined,
) {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const every = pollEveryMs(started);
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- each poll would blank the code the User is reading with a skeleton
  const poll = useQuery(
    UserService.method.pollWatchSyncLink,
    started === undefined ? skipToken : { pluginId, deviceCode: started.deviceCode },
    {
      // A failed poll stops: the tracker may have handed out its tokens once already, so asking
      // again would only fail again until the User gets a new code.
      refetchInterval: (query) =>
        query.state.error === null && query.state.data?.state === WatchSyncLinkState.PENDING
          ? every
          : false,
      // The code is entered on another site, often in another tab.
      refetchIntervalInBackground: true,
      // Asked first after one interval, as the tracker asks, not as the code shows.
      initialData: create(PollWatchSyncLinkResponseSchema, { state: WatchSyncLinkState.PENDING }),
      staleTime: Number.POSITIVE_INFINITY,
    },
  );
  const state = poll.data?.state ?? WatchSyncLinkState.PENDING;
  const linked = state === WatchSyncLinkState.LINKED;
  useEffect(() => {
    if (linked) {
      void queryClient.invalidateQueries({ queryKey: personalKey(transport) });
    }
  }, [linked, queryClient, transport]);
  return { state, error: poll.error };
}
