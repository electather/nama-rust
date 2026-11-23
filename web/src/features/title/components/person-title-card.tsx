import { Link } from "@tanstack/react-router";

import type { SimilarTitle } from "@/features/title/lib/similar-titles";
import { Poster } from "@/shared/components/poster/poster";
import { posterLink } from "@/shared/lib/poster-classes";
import { metadataLine } from "@/shared/lib/title-facts";

/** A title on a Person's page: its poster and name over its year and their `parts` in it. */
export function PersonTitleCard({
  title,
  parts,
}: {
  title: SimilarTitle;
  parts: string | undefined;
}) {
  return (
    <li>
      <Link {...title.link} className={posterLink}>
        <Poster
          url={title.posterUrl}
          thumbhash={title.posterThumbhash}
          title={title.title}
          detail={metadataLine(title.year, parts)}
        />
      </Link>
    </li>
  );
}
