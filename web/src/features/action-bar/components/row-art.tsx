import { RowIcon } from "@/features/action-bar/components/row-icon";
import type { Row } from "@/features/action-bar/lib/menu";
import { useSrcSet } from "@/shared/hooks/use-src-set";

/** A row's 32px art: its poster, its icon in a tile, or a blank so labels line up. */
export function RowArt({ art }: { art: Row["art"] }) {
  const srcSet = useSrcSet(art?.posterUrl);
  if (art === undefined) {
    return null;
  }
  if (art.posterUrl !== undefined) {
    return (
      <span className="grid size-8 shrink-0 overflow-hidden rounded-sm bg-muted">
        <img
          src={art.posterUrl}
          srcSet={srcSet}
          sizes="32px"
          alt=""
          loading="lazy"
          className="size-full object-cover"
        />
      </span>
    );
  }
  return (
    <span className="grid size-8 shrink-0 place-items-center rounded-sm border bg-muted text-foreground-soft group-aria-selected:text-foreground">
      <RowIcon name={art.icon} />
    </span>
  );
}
