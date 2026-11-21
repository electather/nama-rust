import type { LinkOptions } from "@tanstack/react-router";

import { BackLink } from "@/features/player/components/back-link";
import { PlayOtherVersion } from "@/features/player/components/play-other-version";

/** Play `other`, the title's Version that plays here, if there is one, or go `back`. */
export function CantPlayActions({
  other,
  back,
}: {
  other: { id: bigint; picture: string | undefined } | undefined;
  back: LinkOptions;
}) {
  return (
    <div className="flex flex-wrap justify-center gap-2.5">
      {other && <PlayOtherVersion {...other} />}
      <BackLink back={back} variant="secondary">
        Back
      </BackLink>
    </div>
  );
}
