import { MoreReasons } from "@/features/player/components/more-reasons";
import { PlaybackTracks } from "@/features/player/components/playback-tracks";
import { Reasons } from "@/features/player/components/reasons";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import { methodName, wordReasons } from "@/features/player/lib/playback-wording";
import type { Track } from "@/gen/nama/v1/browse_pb";

/** How the Playback Plan plays, why, and with which Tracks. */
export function PlaybackInfo({
  planned,
  tracks,
}: {
  planned: ReturnType<typeof usePlaybackPlan>;
  tracks: Track[];
}) {
  const { decided, context } = wordReasons(planned.plan.reasons);
  return (
    <section aria-label="Playback info" className="min-h-0 overflow-y-auto px-4 pt-1 pb-4">
      <h2 className="text-lg font-semibold tracking-tight">{methodName(planned.plan.method)}</h2>
      <Reasons texts={decided} />
      <MoreReasons texts={context} />
      <PlaybackTracks planned={planned} tracks={tracks} />
    </section>
  );
}
