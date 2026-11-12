import { useNavigate, useRouteContext, useSearch } from "@tanstack/react-router";

import { DeleteMedia, MayDownloadAgain } from "@/components/delete-media";
import type { Episode, Season } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { deletedSearch, seasonName, titleParam } from "@/lib/browse";
import { may } from "@/lib/permissions";

/** Whether the signed-in User may delete media. */
function useMayDelete() {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return may(signedIn.user, Permission.DELETE_MEDIA);
}

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
  if (!useMayDelete()) return null;
  const toLibrary = (deleted: string, notTold: string[]) =>
    navigate({ to: "/libraries/$libraryId", params, search: deletedSearch(deleted, notTold) });
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
        onDeletedEveryVersion={(notTold) =>
          other
            ? navigate({
                to: "/libraries/$libraryId/shows/$showId",
                params: { ...params, showId: titleParam(showId) },
                search: { season: other.number, ...deletedSearch(`${title} ${season}`, notTold) },
              })
            : toLibrary(`${title} ${season}`, notTold)
        }
      />
      <DeleteMedia
        action="Delete Show"
        title={title}
        versions={[]}
        target={() => ({ target: { case: "show", value: { libraryId, showId } } })}
        onDeletedEveryVersion={(notTold) => toLibrary(title, notTold)}
      />
      <NotToldNotice />
    </>
  );
}

/**
 * With Delete media, deletes an Episode's every Version, or one of them. Its row goes with its
 * last Version, so the page's URL then says which Fulfillers may download it again.
 */
export function DeleteEpisode({
  libraryId,
  episodeId,
  episode,
}: {
  libraryId: bigint;
  episodeId: ExternalId;
  episode: Episode;
}) {
  const navigate = useNavigate({ from: "/libraries/$libraryId/shows/$showId/" });
  if (!useMayDelete()) return null;
  return (
    <DeleteMedia
      name={`Delete ${episode.title}`}
      title={episode.title}
      versions={episode.versions}
      target={(versionId) => ({
        target: { case: "episode", value: { libraryId, episodeId, versionId } },
      })}
      onDeletedEveryVersion={(notTold) =>
        navigate({ search: (shown) => ({ ...shown, ...deletedSearch(episode.title, notTold) }) })
      }
    />
  );
}

/** Says which Fulfillers may download what a delete on the Show's page took again, as its URL says. */
function NotToldNotice() {
  const { deleted, notTold = [] } = useSearch({
    from: "/_member/libraries/$libraryId/shows/$showId/",
  });
  return deleted === undefined ? null : <MayDownloadAgain title={deleted} fulfillers={notTold} />;
}
