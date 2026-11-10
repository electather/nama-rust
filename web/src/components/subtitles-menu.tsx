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
 * Picks which of the `<video>`'s subtitles shows, or none, saying which is still being read out of
 * the file. Absent when the plan offers none.
 */
export function SubtitlesMenu({
  subtitles,
  videoRef,
  container,
}: {
  subtitles: PlaybackSubtitle[];
  videoRef: RefObject<HTMLVideoElement | null>;
  /** Where the menu opens: inside the player, so it shows in full screen. */
  container: RefObject<HTMLDivElement | null>;
}) {
  const { shown, states, show } = useSubtitles(videoRef);
  if (subtitles.length === 0) return null;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="overlay" size="icon" aria-label="Subtitles" />}>
        <Captions aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <DropdownMenuRadioGroup
          value={shown ?? ""}
          onValueChange={(url: string) => {
            show(url === "" ? undefined : url);
          }}
        >
          <DropdownMenuRadioItem value="">Off</DropdownMenuRadioItem>
          {subtitles.map(({ track, url }) => (
            <SubtitleItem key={url} track={track} url={url} state={states[url]} />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

const stateText: Record<SubtitleState, string> = {
  loading: "Loading…",
  ready: "",
  failed: "Couldn’t load",
};

function SubtitleItem({
  track,
  url,
  state,
}: {
  track: Track | undefined;
  url: string;
  state: SubtitleState | undefined;
}) {
  const { name, details } = trackLabel(track);
  const more = [details, state === undefined ? "" : stateText[state]]
    .filter((text) => text !== "")
    .join(" · ");
  return (
    <DropdownMenuRadioItem value={url}>
      {name}
      {more !== "" && <span className="text-muted-foreground"> {more}</span>}
    </DropdownMenuRadioItem>
  );
}
