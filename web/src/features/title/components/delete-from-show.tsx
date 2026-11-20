import { useNavigate } from "@tanstack/react-router";
import type { RefObject } from "react";

import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { DeleteMedia } from "@/shared/components/delete-media/delete-media";
import { deletedSearch } from "@/shared/lib/deleted-search";
import type { Opener } from "@/shared/lib/opener";
import { seasonName } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/**
 * The dialogs that delete the Season the row is on, `current`, and the whole Show. Each says after
 * which Fulfillers may download what went again: on the Show, or on its Library once it's gone.
 */
export function DeleteFromShow({
  libraryId,
  showId,
  title,
  seasons,
  current,
  seasonOpener,
  showOpener,
}: {
  libraryId: bigint;
  showId: ExternalId;
  title: string;
  seasons: number[];
  current: number;
  seasonOpener: RefObject<Opener | null>;
  showOpener: RefObject<Opener | null>;
}) {
  const navigate = useNavigate();
  // Without Delete media, the page holds no dialogs it can't open.
  if (!useMayDelete()) {
    return null;
  }
  const params = { libraryId: String(libraryId) };
  const toLibrary = async (deleted: string, notTold: string[]) => {
    await navigate({
      to: "/libraries/$libraryId",
      params,
      search: deletedSearch(deleted, notTold),
    });
  };
  const season = `${title} ${seasonName(current)}`;
  const stay = async (notTold: string[]) => {
    await navigate({
      to: "/libraries/$libraryId/shows/$showId",
      params: { ...params, showId: titleParam(showId) },
      search: deletedSearch(season, notTold),
    });
  };
  return (
    <>
      <DeleteMedia
        ref={seasonOpener}
        action={`Delete ${seasonName(current)}`}
        title={season}
        versions={[]}
        target={() => ({
          target: { case: "season", value: { libraryId, showId, seasonNumber: current } },
        })}
        onDeletedEveryVersion={async (notTold) => {
          await (seasons.length > 1 ? stay(notTold) : toLibrary(season, notTold));
        }}
      />
      <DeleteMedia
        ref={showOpener}
        action="Delete Show"
        title={title}
        versions={[]}
        target={() => ({ target: { case: "show", value: { libraryId, showId } } })}
        onDeletedEveryVersion={async (notTold) => {
          await toLibrary(title, notTold);
        }}
      />
    </>
  );
}
