import { Thumbhash } from "@/shared/components/thumbhash";
import { useServerUrl } from "@/shared/hooks/use-server-url";
import { useSrcSet } from "@/shared/hooks/use-src-set";

/** The backdrop at `url`, under its `thumbhash` until it loads. Nothing without one. */
export function TitleBackdrop({
  url,
  thumbhash,
}: {
  url: string | undefined;
  thumbhash: Uint8Array | undefined;
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
        sizes="100vw"
        alt=""
        // The page's largest image, so it loads first.
        fetchPriority="high"
        className="absolute inset-0 -z-10 size-full object-cover"
      />
    </>
  );
}
