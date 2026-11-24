import { Thumbhash } from "@/shared/components/thumbhash";
import { useSrcSet } from "@/shared/hooks/use-src-set";

/** The backdrop at `url`, under its `thumbhash` until it loads. Nothing without one. */
export function TitleBackdrop({
  url,
  thumbhash,
}: {
  url: string | undefined;
  thumbhash: Uint8Array | undefined;
}) {
  const srcSet = useSrcSet(url, "backdropWidths");
  if (url === undefined) {
    return null;
  }
  return (
    <>
      <Thumbhash thumbhash={thumbhash} />
      <img
        src={url}
        srcSet={srcSet}
        sizes="100vw"
        alt=""
        // The page's largest image, so it loads first.
        fetchPriority="high"
        className="absolute inset-0 -z-10 size-full object-cover"
      />
    </>
  );
}
