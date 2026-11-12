import { Thumbhash } from "@/components/thumbhash";
import type { Library } from "@/gen/nama/v1/library_pb";
import { useSrcSet } from "@/lib/image-sizes";
import { kindName } from "@/lib/libraries";

/**
 * A Library's Backdrop with its name over it, its thumbhash until it loads, or without one a
 * plain box of the same size, so the row doesn't shift. Art is always shown in the dark scheme,
 * so the name keeps the scrim's 70% black and light ink over a light image too (DESIGN.md, "The
 * Legible Over Art Rule").
 */
export function LibraryCard({ library }: { library: Library }) {
  const art = library.backdropUrl !== undefined;
  const srcSet = useSrcSet(library.backdropUrl);
  return (
    <>
      <div
        className="group dark relative isolate flex aspect-video items-end overflow-hidden rounded-lg border bg-muted"
        data-art={art ? "" : undefined}
      >
        <Thumbhash thumbhash={library.backdropThumbhash} />
        {art && (
          // Its name is drawn over it, so the image itself says nothing more.
          <img
            src={library.backdropUrl}
            srcSet={srcSet}
            sizes="(min-width: 1024px) 315px, (min-width: 640px) 50vw, 100vw"
            alt=""
            loading="lazy"
            className="absolute inset-0 size-full object-cover"
          />
        )}
        <div className="relative w-full">
          {art && <div className="h-8 bg-linear-to-t from-scrim" />}
          {/* On the scrim's full strength however many lines it takes, under a fade into the art. */}
          <p className="px-4 pb-3 text-2xl font-semibold tracking-tight text-balance text-foreground group-data-art:bg-scrim">
            {library.name}
          </p>
        </div>
      </div>
      <p className="mt-1.5 text-sm text-muted-foreground">{kindName(library.kind)}</p>
    </>
  );
}
