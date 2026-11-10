import { Captions } from "lucide-react";
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
import type { PlaybackSubtitle } from "@/gen/nama/v1/playback_pb";
import type { SubtitleChoice, usePlaybackPlan } from "@/lib/playback-plan";
import { type SubtitleState, trackLabel, useSubtitles } from "@/lib/subtitles";

/** The `<video>`'s `<track>`s, one per subtitle, which the browser fetches only once shown. */
export function SubtitleTracks({ subtitles }: { subtitles: PlaybackSubtitle[] }) {
  return subtitles.map(({ track, url }) => (
    <track
      key={url}
      kind="subtitles"
      src={url}
      srcLang={track?.language}
      label={browserMenuLabel(track)}
    />
  ));
}

/** What the browser's own subtitle menu, such as in an iPhone's full screen, calls it. */
function browserMenuLabel(track: Track | undefined) {
  const { name, details } = trackLabel(track);
  return details === "" ? name : `${name} (${details})`;
}

/**
 * Picks the subtitle shown, or none: one of the plan's text subtitles, which the `<video>` shows
 * itself, saying which is still being read out of the file, or one of the Version's `tracks` to
 * burn into the picture, an image one or an ASS one with its styling. Absent when there's none to
 * pick.
 */
export function SubtitlesMenu({
  tracks,
  planned: { plan, shownSubtitle: shown, subtitleTrack: burnedIn, chooseSubtitle },
  videoRef,
  container,
}: {
  tracks: Track[];
  planned: ReturnType<typeof usePlaybackPlan>;
  videoRef: RefObject<HTMLVideoElement | null>;
  /** Where the menu opens: inside the player, so it shows in full screen. */
  container: RefObject<HTMLDivElement | null>;
}) {
  const { subtitles } = plan;
  const states = useSubtitles(videoRef, shown);
  const burnable = tracks.flatMap((track, place) => (burnsIn(track) ? [place] : []));
  if (subtitles.length + burnable.length === 0) return null;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="overlay" size="icon" aria-label="Subtitles" />}>
        <Captions aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <DropdownMenuRadioGroup
          value={shown ?? burnedIn ?? ""}
          onValueChange={(choice: SubtitleChoice) => {
            if (videoRef.current)
              chooseSubtitle(videoRef.current, choice === "" ? undefined : choice);
          }}
        >
          <DropdownMenuRadioItem value="">Off</DropdownMenuRadioItem>
          {subtitles.map(({ track, url }) => (
            <SubtitleItem key={url} track={track} value={url} more={stateText(states[url])} />
          ))}
          {burnable.map((place) => (
            <BurnInItem key={place} track={tracks[place]} place={place} />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** Whether `track` is a subtitle the Server burns in: an image one, such as PGS, or ASS or SSA. */
function burnsIn(track: Track) {
  const text = track.kind.case === "subtitle" && ["ass", "ssa"].includes(track.codec);
  return text || isImageSubtitle(track);
}

function isImageSubtitle(track: Track | undefined) {
  return track?.kind.case === "subtitle" && track.kind.value.image;
}

const stateTexts: Record<SubtitleState, string> = {
  loading: "Loading…",
  ready: "",
  failed: "Couldn’t load",
};

function stateText(state: SubtitleState | undefined) {
  return state === undefined ? "" : stateTexts[state];
}

/** The item burning in `track`, at `place` in the Version's Tracks. */
function BurnInItem({ track, place }: { track: Track | undefined; place: number }) {
  // An image one has no styling to tell it from.
  const more = isImageSubtitle(track) ? "" : "With styling";
  return <SubtitleItem track={track} value={place} more={more} />;
}

/** `track`'s item, choosing `value`, with `more` said after its details. */
function SubtitleItem({
  track,
  value,
  more: after,
}: {
  track: Track | undefined;
  value: SubtitleChoice;
  more: string;
}) {
  const { name, details } = trackLabel(track);
  const more = [details, after].filter((text) => text !== "").join(" · ");
  return (
    <DropdownMenuRadioItem value={value}>
      {name}
      {more !== "" && <span className="text-muted-foreground"> {more}</span>}
    </DropdownMenuRadioItem>
  );
}
