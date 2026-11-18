import { useSrcSet } from "@/shared/hooks/use-src-set";

/** One artwork to pick, at the smallest copy the Server resizes to that fits `sizes`. */
export function ChoiceImage({ url, sizes }: { url: string; sizes: string }) {
  const srcSet = useSrcSet(url);
  // The button names it, so the image itself says nothing more.
  return (
    <img
      src={url}
      srcSet={srcSet}
      sizes={sizes}
      alt=""
      loading="lazy"
      className="size-full object-cover"
    />
  );
}
