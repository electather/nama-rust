import { ChangeRow } from "@/features/player/components/change-row";
import { InfoRow } from "@/features/player/components/info-row";
import { type Planned, playedTracks } from "@/features/player/lib/played-tracks";
import type { Track } from "@/gen/nama/v1/browse_pb";

/** The video and audio a plan plays, from what the file holds, then its language and the subtitles. */
export function PlaybackTracks({ planned, tracks }: { planned: Planned; tracks: Track[] }) {
  const { video, audio, language, subtitle } = playedTracks(planned, tracks);
  return (
    <dl className="mt-5 space-y-1">
      <ChangeRow name="Video" change={video} />
      <ChangeRow name="Audio" change={audio} />
      <InfoRow name="Language">{language}</InfoRow>
      <InfoRow name="Subtitles">{subtitle}</InfoRow>
    </dl>
  );
}
