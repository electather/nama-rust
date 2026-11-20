import { useQuery } from "@connectrpc/connect-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** The User's Subtitle Appearance, each part `undefined` until the Server says. */
export function useSubtitleAppearance() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Version plays while the Server answers, its cues in the default appearance
  const { data } = useQuery(PlaybackService.method.getSubtitleAppearance, {});
  return { size: data?.size, background: data?.background };
}
