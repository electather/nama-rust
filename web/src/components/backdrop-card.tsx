import type { ReactNode } from "react";

import { Thumbhash } from "@/components/thumbhash";
import { ResumeBar } from "@/components/watch-state";
import type { Library } from "@/gen/nama/v1/library_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { useSrcSet } from "@/lib/image-sizes";
import { kindName } from "@/lib/libraries";

/** A grid of `BackdropCard`s: one column on a phone, up to three on a desktop. */
export function BackdropGrid({ children }: { children: ReactNode }) {
  return <ul className="mt-4 grid gap-x-4 gap-y-6 sm:grid-cols-2 lg:grid-cols-3">{children}</ul>;
}

/** A Library's Backdrop with its name over it and its kind under it. */
export function LibraryCard({ library }: { library: Library }) {
  return (
    <BackdropCard
      url={library.backdropUrl}
      thumbhash={library.backdropThumbhash}
      title={library.name}
      detail={kindName(library.kind)}
    />
  );
}

/**
 * A Backdrop with a title and your Watch State over it, its thumbhash until it loads, or without
 * one a plain box of the same size, so the grid doesn't shift, and a line of `detail` under it.
 * Art is always shown in the dark scheme, so the title keeps the scrim's 70% black and light ink
 * over a light image too (DESIGN.md, "The Legible Over Art Rule").
 */
export function BackdropCard({
  url,
  thumbhash,
  title,
  detail,
  watchState,
}: {
  url: string | undefined;
  thumbhash: Uint8Array | undefined;
  title: string;
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
            // A column of `BackdropGrid` at each of its breakpoints.
            sizes="(min-width: 1024px) 315px, (min-width: 640px) 50vw, 100vw"
            alt=""
            loading="lazy"
            className="absolute inset-0 size-full object-cover"
          />
        )}
        <div className="relative w-full">
          {art && <div className="h-8 bg-linear-to-t from-scrim" />}
          {/* On the scrim's full strength however many lines it takes, under a fade into the art. */}
          <div className="px-4 pb-3 group-data-art:bg-scrim">
            <p className="text-2xl font-semibold tracking-tight text-balance text-foreground">
              {title}
            </p>
            <ResumeBar state={watchState} />
          </div>
        </div>
      </div>
      {/* Always there, so the cards in a row line up whether or not each has a detail. */}
      <p className="mt-1.5 text-sm text-muted-foreground">{detail}</p>
    </>
  );
}
