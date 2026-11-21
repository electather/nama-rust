import type { SimilarTitle } from "@/features/title/lib/similar-titles";
import { ShelfPoster } from "@/shared/components/shelf-poster";

/** One Similar title in its row: its poster alone, as Home's rows draw them, opening its page. */
export function SimilarPoster({ title }: { title: SimilarTitle }) {
  return (
    <li className="w-tile-poster shrink-0 snap-start">
      <ShelfPoster
        link={title.link}
        title={title.title}
        art={{ url: title.posterUrl, thumbhash: title.posterThumbhash }}
      />
    </li>
  );
}
