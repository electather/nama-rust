import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { Thumbhash } from "@/shared/components/thumbhash";
import { PlayedMark } from "@/shared/components/watch-state/played-mark";
import { WatchedBar } from "@/shared/components/watch-state/watched-bar";
import { useSrcSet } from "@/shared/hooks/use-src-set";

/**
 * A title's poster with its name and a line of metadata under it, and your Watch State: a played
 * mark on the art, or a bar under it once you've started. The 2:3 box is there before the image
 * loads, so the grid doesn't shift, showing its `thumbhash` until then.
 */
export function Poster({
  url,
  thumbhash,
  title,
  detail,
  watchState,
}: {
  url?: string | undefined;
  thumbhash?: Uint8Array | undefined;
  title: string;
  detail?: string | undefined;
  watchState?: WatchState | undefined;
}) {
  const srcSet = useSrcSet(url);
  return (
    <>
      <div className="relative isolate aspect-2/3 overflow-hidden rounded-lg border bg-muted">
        <Thumbhash thumbhash={thumbhash} />
        {url !== undefined && (
          // Its title is the caption, so the image itself says nothing more.
          <img
            src={url}
            srcSet={srcSet}
            // A column of `PosterGrid` at each of its breakpoints.
            sizes="(min-width: 1024px) 183px, (min-width: 768px) 22vw, (min-width: 640px) 30vw, 45vw"
            alt=""
            loading="lazy"
            className="size-full object-cover"
          />
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
