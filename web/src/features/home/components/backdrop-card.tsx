import { CardCaption } from "@/features/home/components/card-caption";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { Thumbhash } from "@/shared/components/thumbhash";
import { useSrcSet } from "@/shared/hooks/use-src-set";
import { backdropGridSizes } from "@/shared/lib/backdrop-grid-sizes";

/**
 * A Backdrop with a title, as its Title Logo if it has one, and your Watch State over it, its
 * thumbhash until it loads, or without one a plain box of the same size, so the grid doesn't
 * shift, and a line of `detail` under it. Art is always shown in the dark scheme, so the title
 * keeps the scrim's 70% black and light ink over a light image too (DESIGN.md, "The Legible Over
 * Art Rule").
 */
export function BackdropCard({
  url,
  thumbhash,
  title,
  logoUrl,
  detail,
  watchState,
}: {
  url: string | undefined;
  thumbhash: Uint8Array | undefined;
  title: string;
  logoUrl?: string | undefined;
  detail: string | undefined;
  watchState?: WatchState | undefined;
}) {
  const art = url !== undefined;
  const srcSet = useSrcSet(url);
  return (
    <>
      <div
        className="group dark relative isolate flex aspect-video items-end overflow-hidden rounded-lg border bg-muted"
        data-art={art ? "" : undefined}
      >
        <Thumbhash thumbhash={thumbhash} />
        {art && (
          // Its title is drawn over it, so the image itself says nothing more.
          <img
            src={url}
            srcSet={srcSet}
            sizes={backdropGridSizes}
            alt=""
            loading="lazy"
            className="absolute inset-0 size-full object-cover"
          />
        )}
        <CardCaption art={art} title={title} logoUrl={logoUrl} watchState={watchState} />
      </div>
      {/* Always there, so the cards in a row line up whether or not each has a detail. */}
      <p className="mt-1.5 text-sm text-muted-foreground">{detail}</p>
    </>
  );
}
