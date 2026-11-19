import { useSrcSet } from "@/shared/hooks/use-src-set";

/** A title's whole poster, 2:3 and uncropped, as wide as the slot an icon row leads with. */
export function RowArt({ poster }: { poster: string }) {
  const srcSet = useSrcSet(poster);
  return (
    <span className="flex w-10 shrink-0 justify-center">
      {/* The row's label names the title, so the image says nothing more. */}
      <img
        src={poster}
        srcSet={srcSet}
        sizes="38px"
        alt=""
        loading="lazy"
        className="aspect-2/3 h-14 rounded-xs bg-muted object-cover outline -outline-offset-1 outline-border"
      />
    </span>
  );
}
