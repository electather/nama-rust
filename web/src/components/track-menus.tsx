import { AudioLines } from "lucide-react";
import type { RefObject } from "react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { metadataLine, soundOf } from "@/lib/browse";
import { PlaybackLimitMenu } from "@/components/playback-limit-menu";
import { SubtitlesMenu } from "@/components/subtitles-menu";
import type { usePlaybackPlan } from "@/lib/playback-plan";
import { trackLabel } from "@/lib/subtitles";

/**
 * The menus choosing what plays: the Version's audio Track, then its subtitles, then the Playback
 * Limit.
 */
export function TrackMenus({
  tracks,
  planned,
  videoRef,
  container,
}: {
  tracks: Track[];
  planned: ReturnType<typeof usePlaybackPlan>;
  videoRef: RefObject<HTMLVideoElement | null>;
  container: RefObject<HTMLDivElement | null>;
}) {
  return (
    <>
      <AudioMenu
        tracks={tracks}
        offered={planned.plan.audioTracks}
        playing={planned.plan.audioTrack}
        choose={(place) => {
          if (videoRef.current) planned.chooseAudio(videoRef.current, place);
        }}
        container={container}
      />
      <SubtitlesMenu tracks={tracks} planned={planned} videoRef={videoRef} container={container} />
      <PlaybackLimitMenu
        offered={planned.plan.playbackLimits}
        chosen={planned.plan.playbackLimit}
        choose={(limit) => {
          if (videoRef.current) planned.chooseLimit(videoRef.current, limit);
        }}
        container={container}
      />
    </>
  );
}

/**
 * Picks which of the Version's audio Tracks plays, by its place in `tracks`, with `choose`, from
 * those `offered`. Absent when there's only one to play.
 */
function AudioMenu({
  tracks,
  offered: audio,
  playing,
  choose,
  container,
}: {
  tracks: Track[];
  /** The places of the audio Tracks the plan offers. */
  offered: number[];
  /** The place of the one the plan plays. */
  playing: number | undefined;
  choose: (place: number) => void;
  /** Where the menu opens: inside the player, so it shows in full screen. */
  container: RefObject<HTMLDivElement | null>;
}) {
  if (audio.length < 2) return null;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="overlay" size="icon" aria-label="Audio" />}>
        <AudioLines aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <DropdownMenuRadioGroup
          value={String(playing)}
          onValueChange={(place: string) => {
            choose(Number(place));
          }}
        >
          {audio.map((place) => (
            <AudioItem key={place} place={place} track={tracks[place]} />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function AudioItem({ place, track }: { place: number; track: Track | undefined }) {
  const { name, details } = trackLabel(track);
  const more = metadataLine(details, track && soundOf([track]));
  return (
    <DropdownMenuRadioItem value={String(place)}>
      {name}
      {more !== "" && <span className="text-muted-foreground"> {more}</span>}
    </DropdownMenuRadioItem>
  );
}
