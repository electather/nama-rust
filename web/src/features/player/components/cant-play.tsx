import { BackButton } from "@/features/player/components/back-button";
import { BackLink } from "@/features/player/components/back-link";
import { CantPlayHeading } from "@/features/player/components/cant-play-heading";
import { CantPlayReasons } from "@/features/player/components/cant-play-reasons";
import type { PlayerProps } from "@/features/player/lib/player-props";
import type { PlaybackReason } from "@/gen/nama/v1/playback_pb";

/**
 * Says the Version can't play here, over the title's art: why, as the `reasons` that decided its
 * plan say, or else in general.
 */
export function CantPlay({
  title,
  heading,
  back,
  backdropUrl,
  reasons,
}: Pick<PlayerProps, "title" | "heading" | "back" | "backdropUrl"> & {
  reasons: PlaybackReason[];
}) {
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
        <CantPlayHeading title={heading ?? title} general={reasons.length === 0} />
        <CantPlayReasons reasons={reasons} />
        <BackLink back={back} variant="secondary">
          Back
        </BackLink>
      </main>
    </div>
  );
}
