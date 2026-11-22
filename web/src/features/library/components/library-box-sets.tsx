import { Link } from "@tanstack/react-router";

import { TitleCount } from "@/features/library/components/title-count";
import { TitleGrid } from "@/features/library/components/title-grid";
import { firstBoxSetPage } from "@/features/library/lib/library-view";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { Poster } from "@/shared/components/poster/poster";
import { usePaged } from "@/shared/hooks/use-paged";
import { posterLink } from "@/shared/lib/poster-classes";
import { titleParam } from "@/shared/lib/title-param";

const titles = { one: "Box Set", many: "Box Sets" };

/** A Movies Library's Box Sets, by name, each opening its page. */
export function LibraryBoxSets({ libraryId }: { libraryId: bigint }) {
  const paged = usePaged(BrowseService.method.listBoxSets, firstBoxSetPage(libraryId));
  const posters = paged.data.pages
    .flatMap((page) => page.boxSets)
    .map((boxSet) => ({
      key: titleParam(boxSet.id),
      poster: (
        <Link
          to="/box-sets/$boxSetId"
          params={{ boxSetId: titleParam(boxSet.id) }}
          className={posterLink}
        >
          <Poster url={boxSet.posterUrl} title={boxSet.name} />
        </Link>
      ),
    }));
  if (posters.length === 0) {
    return <p className="mt-8 text-muted-foreground">No Box Sets in this Library yet.</p>;
  }
  return (
    <>
      <TitleCount count={paged.totalSize} titles={titles} />
      <TitleGrid titles={titles} paged={paged} posters={posters} />
    </>
  );
}
