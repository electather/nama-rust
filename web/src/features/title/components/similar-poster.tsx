import { Link } from "@tanstack/react-router";

import type { SimilarTitle } from "@/features/title/lib/similar-titles";
import { Poster } from "@/shared/components/poster/poster";
import { posterLink } from "@/shared/lib/poster-classes";

/** One Similar title in its row: its poster, title and year, opening its page. */
export function SimilarPoster({ title }: { title: SimilarTitle }) {
  return (
    <li className="w-tile-poster shrink-0 snap-start">
      <Link {...title.link} className={posterLink}>
        <Poster
          url={title.posterUrl}
          thumbhash={title.posterThumbhash}
          title={title.title}
          detail={title.year}
        />
      </Link>
    </li>
  );
}
