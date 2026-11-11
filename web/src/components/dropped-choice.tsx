import type { PlaybackLimit, PlaybackPlan } from "@/gen/nama/v1/playback_pb";
import type { Choice } from "@/lib/playback-plan";
import { belowOriginal } from "@/lib/playback-wording";

/** Says when `plan` dropped the choice the User `justChose`, which it couldn't honour (ADR 0031). */
export function DroppedChoice({ justChose, plan }: { justChose: Choice; plan: PlaybackPlan }) {
  const dropped = droppedText(justChose, plan);
  if (dropped === undefined) return null;
  return (
    <div className="mt-2 flex justify-center px-4">
      <output className="max-w-md rounded-lg bg-scrim px-4 py-2 text-center text-sm text-balance">
        {dropped} Playback info says why.
      </output>
    </div>
  );
}

function droppedText({ audioTrack, subtitleTrack, playbackLimit }: Choice, plan: PlaybackPlan) {
  if (isDropped(audioTrack, plan.audioTrack)) return "Can’t play that audio Track here right now.";
  if (isDropped(subtitleTrack, plan.subtitleTrack))
    return "Can’t burn in those subtitles here right now.";
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
