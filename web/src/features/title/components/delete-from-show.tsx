import { useNavigate } from "@tanstack/react-router";

import { NotToldNotice } from "@/features/title/components/not-told-notice";
import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { Season } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { DeleteMedia } from "@/shared/components/delete-media/delete-media";
import { deletedSearch } from "@/shared/lib/deleted-search";
import { seasonName } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/**
 * With Delete media, deletes the Season shown, then shows another, or the whole Show, then leaves
 * for its Library, either saying which Fulfillers may download what went again. Says so too for
 * an Episode whose row went.
 */
export function DeleteFromShow({
  libraryId,
  showId,
  title,
  seasons,
  current,
}: {
  libraryId: bigint;
  showId: ExternalId;
  title: string;
  seasons: Season[];
  current: number;
}) {
  const navigate = useNavigate();
  const params = { libraryId: String(libraryId) };
  if (!useMayDelete()) {
    return null;
  }
  const toLibrary = async (deleted: string, notTold: string[]) => {
    await navigate({
      to: "/libraries/$libraryId",
      params,
      search: deletedSearch(deleted, notTold),
    });
  };
  const other = seasons.find(({ number }) => number !== current);
  const season = seasonName(current);
  return (
    <>
      <DeleteMedia
        action={`Delete ${season}`}
        title={`${title} ${season}`}
        versions={[]}
        target={() => ({
          target: { case: "season", value: { libraryId, showId, seasonNumber: current } },
        })}
        onDeletedEveryVersion={async (notTold) => {
          await (other
            ? navigate({
                to: "/libraries/$libraryId/shows/$showId",
                params: { ...params, showId: titleParam(showId) },
                search: { season: other.number, ...deletedSearch(`${title} ${season}`, notTold) },
              })
            : toLibrary(`${title} ${season}`, notTold));
        }}
      />
      <DeleteMedia
        action="Delete Show"
        title={title}
        versions={[]}
        target={() => ({ target: { case: "show", value: { libraryId, showId } } })}
        onDeletedEveryVersion={async (notTold) => {
          await toLibrary(title, notTold);
        }}
      />
      <NotToldNotice />
    </>
  );
}
