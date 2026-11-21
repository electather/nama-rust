import type { Still } from "@/features/action-bar/lib/rows";
import { Thumbhash } from "@/shared/components/thumbhash";
import { useSrcSet } from "@/shared/hooks/use-src-set";

/** An Episode's Still, 16:9 and as tall as a title's poster, its thumbhash until it loads. */
export function RowStill({ still }: { still: Still }) {
  const srcSet = useSrcSet(still.url);
  return (
    <span className="relative isolate aspect-video h-14 shrink-0 overflow-hidden rounded-xs border bg-muted">
      <Thumbhash thumbhash={still.thumbhash} />
      {/* The row's label names the Episode, so the image says nothing more. */}
      <img
        src={still.url}
        srcSet={srcSet}
        sizes="100px"
        alt=""
        loading="lazy"
        className="size-full object-cover"
      />
    </span>
  );
}
