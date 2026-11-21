import { BackButton } from "@/features/player/components/back-button";
import { CantPlayActions } from "@/features/player/components/cant-play-actions";
import { CantPlayHeading } from "@/features/player/components/cant-play-heading";
import { CantPlayReasons } from "@/features/player/components/cant-play-reasons";
import { otherVersion } from "@/features/player/lib/other-version";
import type { PlayerProps } from "@/features/player/lib/player-props";
import type { PlaybackReason } from "@/gen/nama/v1/playback_pb";

/**
 * Says the Version can't play here, over the title's art: why, as the `reasons` that decided its
 * plan say, or else in general. When one of the title's `playable` Versions plays here, it offers
 * that one.
 */
export function CantPlay({
  title,
  heading,
  back,
  backdropUrl,
  versions,
  reasons,
  playable,
}: Pick<PlayerProps, "title" | "heading" | "back" | "backdropUrl" | "versions"> & {
  reasons: PlaybackReason[];
  playable: bigint[];
}) {
  const other = otherVersion(versions, playable);
  return (
    <div className="fixed inset-0 flex overflow-y-auto bg-background px-5 py-12">
      {backdropUrl !== undefined && (
        <img src={backdropUrl} alt="" className="absolute inset-0 size-full object-cover" />
      )}
      <div aria-hidden className="absolute inset-0 bg-scrim" />
      <div className="absolute inset-x-0 top-0 flex p-5">
        <BackButton back={back} />
      </div>
      <main className="relative m-auto flex w-full max-w-130 flex-col items-center gap-5 text-center">
        <CantPlayHeading title={heading ?? title} line={headingLine(other, reasons)} />
        <CantPlayReasons reasons={reasons} />
        <CantPlayActions other={other} back={back} />
      </main>
    </div>
  );
}

/**
 * What the heading says under it: that the other Version plays, when `other` is there, or else a
 * general why when no reason says why.
 */
function headingLine(
  other: { picture: string | undefined } | undefined,
  reasons: PlaybackReason[],
) {
  if (other) {
    return other.picture === undefined
      ? "Its other Version plays on this Device."
      : `Its other Version, ${other.picture}, plays on this Device.`;
  }
  return reasons.length === 0
    ? "This browser can’t play it. Another browser, or another Version, may play."
    : undefined;
}
