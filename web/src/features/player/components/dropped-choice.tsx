import { TriangleAlert } from "lucide-react";

import type { Choice } from "@/features/player/lib/asking";
import type { PlaybackLimit, PlaybackPlan } from "@/gen/nama/v1/playback_pb";
import { StateLabel } from "@/shared/components/state-label";
import { belowOriginal } from "@/shared/lib/playback-names";

/** Says when `plan` dropped the choice the User `justChose`, which it couldn't honour (ADR 0031). */
export function DroppedChoice({ justChose, plan }: { justChose: Choice; plan: PlaybackPlan }) {
  const dropped = droppedText(justChose, plan);
  if (dropped === undefined) {
    return null;
  }
  return (
    <output className="mt-4 flex px-5">
      <StateLabel
        tone="warning"
        Icon={TriangleAlert}
        label={`${dropped} Playback info says why.`}
      />
    </output>
  );
}

function droppedText({ audioTrack, subtitleTrack, playbackLimit }: Choice, plan: PlaybackPlan) {
  if (isDropped(audioTrack, plan.audioTrack)) {
    return "Can’t play that audio Track here right now.";
  }
  if (isDropped(subtitleTrack, plan.subtitleTrack)) {
    return "Can’t burn in those subtitles here right now.";
  }
  return isDropped(playbackLimit, plan.playbackLimit)
    ? `Can’t play at ${limitName(playbackLimit)} here right now.`
    : undefined;
}

function isDropped(chosen: number | undefined, played: number | undefined) {
  return chosen !== undefined && chosen !== played;
}

function limitName(limit: PlaybackLimit | undefined) {
  return belowOriginal.find((below) => below.limit === limit)?.name ?? "that Playback Limit";
}
