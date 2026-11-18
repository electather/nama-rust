import { useSrcSet } from "@/shared/hooks/use-src-set";

/** A Title Logo, named by `title` so the heading it stands in says it to screen readers. */
export function TitleLogo({ url, title }: { url: string; title: string }) {
  const srcSet = useSrcSet(url);
  return (
    <img
      src={url}
      srcSet={srcSet}
      sizes="(min-width: 640px) 448px, 80vw"
      alt={title}
      className="max-h-28 w-auto max-w-4/5 object-contain sm:max-h-36 sm:max-w-md"
    />
  );
}
