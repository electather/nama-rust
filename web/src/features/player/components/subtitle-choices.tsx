import { RadioGroup, RadioRow } from "nama-ui/components/radio-group";
import type { RefObject } from "react";

import { BurnInItem } from "@/features/player/components/burn-in-item";
import { SubtitleItem } from "@/features/player/components/subtitle-item";
import { SubtitleOffset } from "@/features/player/components/subtitle-offset";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { SubtitleChoice } from "@/features/player/lib/asking";
import type { SubtitleState } from "@/features/player/lib/subtitle-render";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { PickedTrackKind, type PlaybackSubtitle } from "@/gen/nama/v1/playback_pb";

/**
 * Off, the plan's subtitles and the Tracks to burn in, the one shown or burned in chosen, then its
 * Subtitle Offset.
 */
export function SubtitleChoices({
  tracks,
  planned,
  videoRef,
  states,
  reportPick,
}: {
  tracks: Track[];
  planned: ReturnType<typeof usePlaybackPlan>;
  videoRef: RefObject<HTMLVideoElement | null>;
  states: Partial<Record<string, SubtitleState>>;
  reportPick: (kind: PickedTrackKind, track: number | undefined) => void;
}) {
  const { plan, shownSubtitle: shown, chooseSubtitle } = planned;
  const { subtitles, burnInTracks: burnable, subtitleTrack: burnedIn } = plan;
  return (
    <>
      <RadioGroup
        aria-label="Subtitles"
        value={shown ?? burnedIn ?? ""}
        onValueChange={(value) => {
          const choice = isChoice(value) ? value : undefined;
          if (videoRef.current) {
            chooseSubtitle(videoRef.current, choice);
          }
          reportPick(PickedTrackKind.SUBTITLE, pickedPlace(choice, subtitles));
        }}
      >
        <RadioRow value="">Off</RadioRow>
        {subtitles.map(({ track, url }) => (
          <SubtitleItem key={url} track={track} value={url} note={stateText(states[url])} />
        ))}
        {burnable.map((place) => (
          <BurnInItem key={place} track={tracks[place]} place={place} />
        ))}
      </RadioGroup>
      <SubtitleOffset planned={planned} />
    </>
  );
}

/** Whether a row's `value` picks a subtitle: Off's is `""`. */
function isChoice(value: unknown): value is NonNullable<SubtitleChoice> {
  return typeof value === "number" || (typeof value === "string" && value !== "");
}

/** The place in the Version's Tracks of the subtitle `choice` picks, `undefined` for Off. */
function pickedPlace(choice: SubtitleChoice, subtitles: PlaybackSubtitle[]) {
  if (typeof choice === "number") {
    return choice;
  }
  return subtitles.find(({ url }) => url === choice)?.place;
}

const stateTexts: Record<SubtitleState, string> = {
  loading: "Loading…",
  ready: "",
  failed: "Couldn’t load",
};

function stateText(state: SubtitleState | undefined) {
  return state === undefined ? "" : stateTexts[state];
}
