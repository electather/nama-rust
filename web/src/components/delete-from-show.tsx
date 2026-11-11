import { useNavigate, useRouteContext } from "@tanstack/react-router";

import { DeleteMedia } from "@/components/delete-media";
import type { Episode, Season } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { seasonName, titleParam } from "@/lib/browse";
import { may } from "@/lib/permissions";

/** Whether the signed-in User may delete media. */
function useMayDelete() {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return may(signedIn.user, Permission.DELETE_MEDIA);
}

/**
 * With Delete media, deletes the Season shown, then shows another, or the whole Show, then leaves
 * for its Library.
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
  const toLibrary = () => navigate({ to: "/libraries/$libraryId", params });
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
        onDeletedEveryVersion={() =>
          other
            ? navigate({
                to: "/libraries/$libraryId/shows/$showId",
                params: { ...params, showId: titleParam(showId) },
                search: { season: other.number },
              })
            : toLibrary()
        }
      />
      <DeleteMedia
        action="Delete Show"
        title={title}
        versions={[]}
        target={() => ({ target: { case: "show", value: { libraryId, showId } } })}
        onDeletedEveryVersion={toLibrary}
      />
    </>
  );
}

/** With Delete media, deletes an Episode's every Version, or one of them. */
export function DeleteEpisode({
  libraryId,
  episodeId,
  episode,
}: {
  libraryId: bigint;
  episodeId: ExternalId;
  episode: Episode;
}) {
  if (!useMayDelete()) return null;
  return (
    <DeleteMedia
      name={`Delete ${episode.title}`}
      title={episode.title}
      versions={episode.versions}
      target={(versionId) => ({
        target: { case: "episode", value: { libraryId, episodeId, versionId } },
      })}
    />
  );
}
