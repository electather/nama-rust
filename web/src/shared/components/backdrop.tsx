import { Thumbhash } from "@/shared/components/thumbhash";
import { useServerUrl } from "@/shared/hooks/use-server-url";
import { useSrcSet } from "@/shared/hooks/use-src-set";

/**
 * The Backdrop at `url`, under its `thumbhash` until it loads, on the scrim text over it sits on,
 * filling its `relative isolate` box behind what comes after it. `sizes` is how wide it shows, for
 * its `srcset`. Nothing without one.
 */
export function Backdrop({
  url,
  thumbhash,
  sizes,
}: {
  url: string | undefined;
  thumbhash: Uint8Array | undefined;
  sizes: string;
}) {
  const src = useServerUrl(url);
  const srcSet = useSrcSet(url, "backdropWidths");
  if (url === undefined) {
    return null;
  }
  return (
    <>
      <Thumbhash thumbhash={thumbhash} />
      <img
        src={src}
        srcSet={srcSet}
        sizes={sizes}
        alt=""
        // The page's largest image, so it loads first.
        fetchPriority="high"
        className="absolute inset-0 -z-10 size-full object-cover"
      />
      <div aria-hidden className="absolute inset-0 -z-10 title-scrim" />
    </>
  );
}
