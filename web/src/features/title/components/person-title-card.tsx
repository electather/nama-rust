import type { SimilarTitle } from "@/features/title/lib/similar-titles";
import { Poster } from "@/shared/components/poster/poster";
import { posterGridSizes } from "@/shared/lib/poster-classes";
import { metadataLine } from "@/shared/lib/title-facts";

/** A title on a Person's page: its poster, labelled with its year and their `parts` in it. */
export function PersonTitleCard({
  title,
  parts,
}: {
  title: SimilarTitle;
  parts: string | undefined;
}) {
  return (
    <li>
      <Poster
        link={title.link}
        title={title.title}
        detail={metadataLine(title.year, parts)}
        art={title.poster}
        sizes={posterGridSizes}
      />
    </li>
  );
}
