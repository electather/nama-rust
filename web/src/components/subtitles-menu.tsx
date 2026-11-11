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
import { type PlaybackSubtitle, SubtitleFormat } from "@/gen/nama/v1/playback_pb";
import type { SubtitleChoice, usePlaybackPlan } from "@/lib/playback-plan";
import {
  type SubtitleState,
  trackLabel,
  useDrawnSubtitle,
  usePickReporter,
  useSubtitles,
} from "@/lib/subtitles";

/**
 * The `<video>`'s `<track>`s, one per text subtitle, which the browser fetches only once shown.
 * Those it draws aren't among them.
 */
export function SubtitleTracks({ subtitles }: { subtitles: PlaybackSubtitle[] }) {
  const text = subtitles.filter(({ format }) => format === SubtitleFormat.WEBVTT);
  return text.map(({ track, url }) => (
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
 * Picks the subtitle shown, or none: one of the plan's subtitles, which the Device shows itself,
 * saying which is still being read out of the file, or one of the Version's `tracks` the plan
 * offers to burn into the picture, an image one it can't draw or an ASS one it doesn't draw, with
 * its styling. Each pick is reported, so the Server keeps it for the Show or Movie the Version is
 * of. Absent when there's none to pick.
 */
export function SubtitlesMenu({
  tracks,
  planned,
  videoRef,
  container,
}: {
  tracks: Track[];
  planned: ReturnType<typeof usePlaybackPlan>;
  videoRef: RefObject<HTMLVideoElement | null>;
  /** Where the menu opens: inside the player, so it shows in full screen. */
  container: RefObject<HTMLDivElement | null>;
}) {
  const { versionId, plan, shownSubtitle: shown, chooseSubtitle } = planned;
  const { subtitles, burnInTracks: burnable, subtitleTrack: burnedIn } = plan;
  const states = useShownSubtitles(planned, videoRef);
  const reportPick = usePickReporter(versionId);
  if (subtitles.length + burnable.length === 0) return null;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="overlay" size="icon" aria-label="Subtitles" />}>
        <Captions aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <DropdownMenuRadioGroup
          value={shown ?? burnedIn ?? ""}
          onValueChange={(value: SubtitleChoice) => {
            const choice = value === "" ? undefined : value;
            if (videoRef.current) chooseSubtitle(videoRef.current, choice);
            reportPick(pickedPlace(choice, subtitles));
          }}
        >
          <DropdownMenuRadioItem value="">Off</DropdownMenuRadioItem>
          {subtitles.map(({ track, url }) => (
            <SubtitleItem key={url} track={track} value={url} note={stateText(states[url])} />
          ))}
          {burnable.map((place) => (
            <BurnInItem key={place} track={tracks[place]} place={place} />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** The place in the Version's Tracks of the subtitle `choice` picks, `undefined` for Off. */
function pickedPlace(choice: SubtitleChoice, subtitles: PlaybackSubtitle[]) {
  if (typeof choice === "number") return choice;
  return subtitles.find(({ url }) => url === choice)?.place;
}

/**
 * Shows the plan's subtitle at the URL `shownSubtitle`, as a `<track>` or drawn, and says how each
 * shown so far is getting on. One drawn that fails to draw is chosen to burn in instead (ADR 0033).
 */
function useShownSubtitles(
  {
    plan,
    shownSubtitle: shown,
    chooseSubtitle,
  }: Pick<ReturnType<typeof usePlaybackPlan>, "plan" | "shownSubtitle" | "chooseSubtitle">,
  videoRef: RefObject<HTMLVideoElement | null>,
) {
  const drawn = plan.subtitles.find(
    ({ url, format }) => url === shown && format !== SubtitleFormat.WEBVTT,
  );
  return {
    ...useSubtitles(videoRef, shown),
    ...useDrawnSubtitle(videoRef, drawn, () => {
      if (videoRef.current && drawn) chooseSubtitle(videoRef.current, drawn.place);
    }),
  };
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
  const note = isImageSubtitle(track) ? "" : "With styling";
  return <SubtitleItem track={track} value={place} note={note} />;
}

/** `track`'s item, choosing `value`, with `note` said after its details. */
function SubtitleItem({
  track,
  value,
  note,
}: {
  track: Track | undefined;
  value: SubtitleChoice;
  note: string;
}) {
  const { name, details } = trackLabel(track);
  const more = [details, note].filter((text) => text !== "").join(" · ");
  return (
    <DropdownMenuRadioItem value={value}>
      {name}
      {more !== "" && <span className="text-muted-foreground"> {more}</span>}
    </DropdownMenuRadioItem>
  );
}
