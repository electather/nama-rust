import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import { trackChange } from "@/features/player/lib/track-change";
import { trackLabel } from "@/features/player/lib/track-label";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { codecName } from "@/shared/lib/codec-name";

/** What Playback info reads of the plan the player plays. */
export type Planned = Pick<ReturnType<typeof usePlaybackPlan>, "plan" | "shownSubtitle">;

/**
 * What a plan plays, from the Version's `tracks`: its video and audio, as they play and as the file
 * holds them, the audio's language, and the subtitle it plays, shows or burns in, each in words.
 */
export function playedTracks(planned: Planned, tracks: Track[]) {
  const { plan } = planned;
  const audio = firstOf(plan.tracks, "audio");
  return {
    video: changeOf(firstOf(plan.tracks, "video"), firstOf(tracks, "video")),
    audio: changeOf(audio, at(tracks, plan.audioTrack) ?? firstOf(tracks, "audio")),
    language: audio && trackLabel(audio).name,
    subtitle: subtitleWords(playedSubtitle(planned, tracks)),
  };
}

function changeOf(played: Track | undefined, source: Track | undefined) {
  return played && trackChange(played, source);
}

function playedSubtitle({ plan, shownSubtitle }: Planned, tracks: Track[]) {
  const shown = plan.subtitles.find(({ url }) => url === shownSubtitle);
  return firstOf(plan.tracks, "subtitle") ?? shown?.track ?? at(tracks, plan.subtitleTrack);
}

function subtitleWords(subtitle: Track | undefined) {
  return subtitle && `${codecName(subtitle.codec)} · ${trackLabel(subtitle).name}`;
}

function firstOf(tracks: Track[], kind: NonNullable<Track["kind"]["case"]>) {
  return tracks.find((track) => track.kind.case === kind);
}

function at(tracks: Track[], place: number | undefined) {
  return place === undefined ? undefined : tracks[place];
}
