import { EverythingMatched } from "@/features/admin-needs-a-match/components/everything-matched";
import { FilesToRename } from "@/features/admin-needs-a-match/components/files-to-rename";
import { useUnmatchedItems } from "@/features/admin-needs-a-match/data/use-unmatched-items";
import type { Fixing } from "@/features/admin-needs-a-match/lib/fixing";
import { loaded } from "@/features/admin-needs-a-match/lib/loaded";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { UnmatchedRows } from "@/features/settings/components/unmatched-rows";
import type { Library } from "@/gen/nama/v1/library_pb";
import { ShowMore } from "@/shared/components/show-more";

/**
 * `library`'s Unmatched Items a page at a time under its name, if it has any, then the `rest`'s,
 * or that everything is Matched if neither they nor the Libraries before them (`anyBefore`) have
 * any. Each Library is a component of its own, since each pages through its list with a hook.
 */
export function UnmatchedLibrary({
  library,
  rest: [next, ...rest],
  anyBefore,
  fixing,
  toggle,
}: { library: Library; rest: Library[]; anyBefore: boolean } & Fixing) {
  const { data, hasNextPage, fetchNextPage } = useUnmatchedItems(library);
  const { items, unparsedPaths, needsAMatch, anyYet } = loaded(data.pages, hasNextPage, anyBefore);
  return (
    <>
      {needsAMatch && (
        <SettingGroup title={library.name}>
          <UnmatchedRows items={items} libraryId={library.id} fixing={fixing} toggle={toggle} />
          {hasNextPage && <ShowMore load={fetchNextPage} loaded={data.pages.length} />}
          <FilesToRename paths={unparsedPaths} />
        </SettingGroup>
      )}
      {next === undefined ? (
        <EverythingMatched anyListed={anyYet} />
      ) : (
        <UnmatchedLibrary
          library={next}
          rest={rest}
          anyBefore={anyYet}
          fixing={fixing}
          toggle={toggle}
        />
      )}
    </>
  );
}
