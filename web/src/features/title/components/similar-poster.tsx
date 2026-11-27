import type { SimilarTitle } from "@/features/title/lib/similar-titles";
import { Poster } from "@/shared/components/poster/poster";
import { posterTile } from "@/shared/lib/tile-shapes";

/** One Similar title in its row: its poster alone, as Home's rows draw them, opening its page. */
export function SimilarPoster({ title }: { title: SimilarTitle }) {
  return (
    <li
      className={`shrink-0 snap-start ${posterTile.widthClass}`}
      style={{ "--tile-width": posterTile.width }}
    >
      <Poster link={title.link} title={title.title} detail={title.year} art={title.poster} />
    </li>
  );
}
