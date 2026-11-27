import { Link, type LinkOptions } from "@tanstack/react-router";

import { PosterProgress } from "@/features/library/components/poster-progress";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { Poster } from "@/shared/components/poster/poster";
import { PlayedMark } from "@/shared/components/watch-state/played-mark";
import { watchedPercent } from "@/shared/lib/watch-progress";

/** The `sizes` of a poster in a Library's grid: columns of at least 160px, at most 1,400px wide. */
const gridSizes = "(min-width: 1440px) 220px, (min-width: 768px) 20vw, 45vw";

/**
 * A title in a Library's grid: its poster, marked once played or with how far through it the User
 * is, then its name and `detail`, such as its year, under it. Both open `link`; the caption's is
 * left out of the tab order and assistive technology, since the poster's says the same.
 */
export function PosterTile({
  link,
  title,
  detail,
  art,
  state,
}: {
  link: LinkOptions;
  title: string;
  detail?: string | undefined;
  art: { url: string | undefined; thumbhash: Uint8Array | undefined };
  state?: WatchState | undefined;
}) {
  return (
    <div className="flex min-w-0 flex-col gap-2.5">
      <Poster link={link} title={title} detail={detail} art={art} sizes={gridSizes}>
        <PlayedMark state={state} />
        <PosterProgress percent={watchedPercent(state)} />
      </Poster>
      <Link {...link} tabIndex={-1} aria-hidden className="flex min-w-0 flex-col">
        <span className="truncate font-medium">{title}</span>
        {detail !== undefined && (
          <span className="text-xs text-muted-foreground tabular-nums">{detail}</span>
        )}
      </Link>
    </div>
  );
}
