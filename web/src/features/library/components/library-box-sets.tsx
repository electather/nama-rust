import { RailLayout } from "@/features/library/components/rail-layout";
import { TitleGrid } from "@/features/library/components/title-grid";
import { firstBoxSetPage } from "@/features/library/lib/library-view";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { Poster } from "@/shared/components/poster/poster";
import { usePaged } from "@/shared/hooks/use-paged";
import { posterOf } from "@/shared/lib/server-image";
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
        <Poster
          size="tile"
          link={{ to: "/box-sets/$boxSetId", params: { boxSetId: titleParam(boxSet.id) } }}
          title={boxSet.name}
          art={posterOf(boxSet)}
        />
      ),
    }));
  if (posters.length === 0) {
    return (
      <div className="flex flex-col gap-1 py-12">
        <p className="font-medium">No Box Sets yet</p>
        <p className="max-w-prose text-sm text-muted-foreground">
          A Box Set shows up here once this Library holds two or more Movies of the same collection.
        </p>
      </div>
    );
  }
  return (
    <RailLayout>
      <TitleGrid titles={titles} paged={paged} posters={posters} />
    </RailLayout>
  );
}
