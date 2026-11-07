import type { ReactNode } from "react";

import { PlayedMark, WatchedBar } from "@/components/watch-state";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/** The classes for a link wrapping a `Poster`, which shows focus around it. */
export const posterLink =
  "block rounded-lg outline-none focus-visible:ring-3 focus-visible:ring-ring/50";

/** A grid of posters, or what to say instead when there are none. */
export function PosterGrid({ empty, children }: { empty: ReactNode; children: ReactNode[] }) {
  if (children.length === 0) return empty;
  return (
    <ul className="mt-8 grid grid-cols-2 gap-x-4 gap-y-8 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
      {children}
    </ul>
  );
}

/**
 * A title's poster with its name and a line of metadata under it, and your Watch State: a played
 * mark on the art, or a bar under it once you've started. The 2:3 box is there before the image
 * loads, so the grid doesn't shift.
 */
export function Poster({
  url,
  title,
  detail,
  watchState,
}: {
  url?: string | undefined;
  title: string;
  detail?: string | undefined;
  watchState?: WatchState | undefined;
}) {
  return (
    <>
      <div className="relative aspect-2/3 overflow-hidden rounded-lg border bg-muted">
        {url !== undefined && (
          // Its title is the caption, so the image itself says nothing more.
          <img src={url} alt="" loading="lazy" className="size-full object-cover" />
        )}
        <PlayedMark state={watchState} />
      </div>
      {/* Always there, so captions line up whether or not a title has a bar. */}
      <div className="mt-1.5 h-0.5">
        <WatchedBar state={watchState} />
      </div>
      <p className="mt-1.5 text-sm font-medium text-balance">{title}</p>
      {detail !== undefined && <p className="text-sm text-muted-foreground">{detail}</p>}
    </>
  );
}
