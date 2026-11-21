import { useRouteContext } from "@tanstack/react-router";

import type { PlaybackPlan } from "@/gen/nama/v1/playback_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/** Whether the User may search for subtitles: they have Download subtitles, and a source is on. */
export function useMaySearchSubtitles(plan: PlaybackPlan) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return plan.canSearchSubtitles && may(signedIn.user, Permission.DOWNLOAD_SUBTITLES);
}
