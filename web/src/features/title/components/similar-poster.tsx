import type { SimilarTitle } from "@/features/title/lib/similar-titles";
import { Poster } from "@/shared/components/poster/poster";

/** One Similar title in its row: its poster alone, as Home's rows draw them, opening its page. */
export function SimilarPoster({ title }: { title: SimilarTitle }) {
  return (
    <li className="w-tile-poster shrink-0 snap-start">
      <Poster
        link={title.link}
        title={title.title}
        detail={title.year}
        art={{ url: title.posterUrl, thumbhash: title.posterThumbhash }}
      />
    </li>
  );
}
