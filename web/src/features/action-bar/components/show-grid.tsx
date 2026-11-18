import type { ShowInLibrary } from "@/gen/nama/v1/browse_pb";
import { PosterGrid } from "@/shared/components/poster/poster-grid";
import { ShowInLibraryItem } from "@/shared/components/titles-in-library/show-in-library-item";
import { titleParam } from "@/shared/lib/title-param";

/** The Shows a search found, as posters. */
export function ShowGrid({ shows }: { shows: ShowInLibrary[] }) {
  return (
    <PosterGrid empty={null}>
      {shows.map((show) => (
        <ShowInLibraryItem key={`${show.libraryId}/${titleParam(show.id)}`} show={show} />
      ))}
    </PosterGrid>
  );
}
