import { CantPlayReason } from "@/features/player/components/cant-play-reason";
import type { PlaybackReason } from "@/gen/nama/v1/playback_pb";

/** The reasons a Version can't play, on a card. */
export function CantPlayReasons({ reasons }: { reasons: PlaybackReason[] }) {
  if (reasons.length === 0) {
    return null;
  }
  return (
    <ul className="flex flex-col gap-3.5 self-stretch rounded-xl border bg-card p-5 text-start text-sm">
      {reasons.map((reason) => (
        <CantPlayReason key={reason.text} reason={reason} />
      ))}
    </ul>
  );
}
