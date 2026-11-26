import type { Panel } from "@/features/home/lib/featured-panels";
import { Thumbhash } from "@/shared/components/thumbhash";
import { useServerUrl } from "@/shared/hooks/use-server-url";
import { useSrcSet } from "@/shared/hooks/use-src-set";

declare module "react" {
  interface CSSProperties {
    /** The point of a Featured panel's art that its crop keeps. */
    "--art-focus"?: string;
  }
}

/** A little above centre, as the artboard frames a Backdrop. */
const artFocus = { "--art-focus": "center 35%" };

/**
 * A Featured panel's Backdrop, wider than the open panel so its drift never shows an edge, grey
 * and dimmed while closed, and the scrim its copy sits on once open.
 */
export function FeaturedArt({ panel, first }: { panel: Panel; first: boolean }) {
  const src = useServerUrl(panel.backdropUrl);
  const srcSet = useSrcSet(panel.backdropUrl, "backdropWidths");
  return (
    <>
      <Thumbhash thumbhash={panel.backdropThumbhash} />
      {panel.backdropUrl !== undefined && (
        // Its title is the copy over it, so the image says nothing more.
        <img
          src={src}
          srcSet={srcSet}
          sizes="(max-width: 720px) 100vw, 64vw"
          alt=""
          // The first panel opens first: it's the page's largest paint.
          fetchPriority={first ? "high" : undefined}
          className="absolute inset-s-1/2 top-0 h-full w-featured-art max-w-none translate-x-(--drift) object-cover object-(--art-focus) grayscale transition-all duration-600 ease-out-expo group-data-open:grayscale-0 motion-reduce:transition-none max-[720px]:inset-s-0 max-[720px]:w-full max-[720px]:translate-none"
          style={artFocus}
        />
      )}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 bg-background opacity-45 transition-opacity duration-600 ease-out-expo group-data-open:opacity-0 motion-reduce:transition-none"
      />
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 opacity-50 transition-opacity duration-600 ease-out-expo group-data-open:opacity-100 motion-reduce:transition-none"
      >
        <div className="absolute inset-0 bg-linear-to-t from-background/92 via-background/55 via-32% to-transparent to-62%" />
        <div className="absolute inset-0 bg-linear-to-r from-background/60 to-transparent to-55%" />
      </div>
    </>
  );
}
