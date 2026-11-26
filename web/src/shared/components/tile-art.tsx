import { Thumbhash } from "@/shared/components/thumbhash";
import { useServerUrl } from "@/shared/hooks/use-server-url";
import { type OfferedWidths, useSrcSet } from "@/shared/hooks/use-src-set";

/**
 * A shelf tile's art, filling its box, its thumbhash until it loads, under a hairline drawn inside
 * its edge. Its tile names it, so the image says nothing more.
 */
export function TileArt({
  url,
  thumbhash,
  look,
}: {
  url: string | undefined;
  thumbhash: Uint8Array | undefined;
  /**
   * How wide it shows and which widths it offers, for its `srcset`, and its tile's corners'
   * radius, for its hairline.
   */
  look: { sizes: string; radius: string; widths?: OfferedWidths | undefined };
}) {
  const src = useServerUrl(url);
  const srcSet = useSrcSet(url, look.widths);
  return (
    <>
      <Thumbhash thumbhash={thumbhash} />
      {url !== undefined && (
        <img
          src={src}
          srcSet={srcSet}
          sizes={look.sizes}
          alt=""
          loading="lazy"
          className="absolute inset-0 size-full object-cover"
        />
      )}
      <span
        aria-hidden
        className={`absolute inset-0 inset-ring inset-ring-border ${look.radius}`}
      />
    </>
  );
}
