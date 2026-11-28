import { Film, Tv } from "lucide-react";

import { TitleKind } from "@/gen/nama/v1/match_pb";
import { posterRing } from "@/shared/lib/tile-shapes";

const SIZES = {
  row: { box: "h-12 w-8 [&_svg]:size-4", radius: "rounded-md" },
  // 80 by 120, and on a phone 56 by 84.
  header: {
    box: "h-21 w-14 md:h-30 md:w-20 [&_svg]:size-5 md:[&_svg]:size-6",
    radius: "rounded-lg md:rounded-xl",
  },
};

/**
 * A stand-in for the poster of a file that has no title yet: a plain Muted box in a poster's
 * shape, with a Movie's film or a Show's screen in it. It's art, so assistive technology skips it.
 */
export function FilePoster({ kind, size }: { kind: TitleKind; size: keyof typeof SIZES }) {
  const Icon = kind === TitleKind.SHOW ? Tv : Film;
  const { box, radius } = SIZES[size];
  return (
    <span
      aria-hidden
      className={`relative flex shrink-0 items-center justify-center bg-muted text-muted-foreground ${box} ${radius}`}
    >
      <Icon strokeWidth={1.5} />
      <span className={`${posterRing} ${radius}`} />
    </span>
  );
}
