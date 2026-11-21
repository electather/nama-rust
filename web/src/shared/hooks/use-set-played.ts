import type { MessageInitShape } from "@bufbuild/protobuf";
import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { type QueryClient, useQueryClient } from "@tanstack/react-query";

import {
  BrowseService,
  type GetMovieResponse,
  type ListEpisodesRequestSchema,
  type ListEpisodesResponse,
} from "@/gen/nama/v1/browse_pb";
import type { ExternalIdSchema } from "@/gen/nama/v1/match_pb";
import { type SetPlayedRequestSchema, WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { browseKey } from "@/shared/hooks/browse";
import { markEpisodes, markMovie } from "@/shared/lib/marked-played";

/** Each `useSetPlayed` mutation's, for a screen to show what's being marked before it's done. */
export const setPlayedKey = [WatchStateService.method.setPlayed.name];

type Asked = MessageInitShape<typeof SetPlayedRequestSchema>;
type Target = Extract<Asked["target"], { case: string }>;

/**
 * Marks a Movie, Episode, Season or Show played or unplayed. The Movie's page and the Episodes on
 * screen show it at once; then everything on screen refetches, so posters and Continue Watching
 * show it too, and what was shown comes back if the Server refuses.
 */
export function useSetPlayed() {
  const queryClient = useQueryClient();
  const { mutate } = useMutation(WatchStateService.method.setPlayed, {
    mutationKey: setPlayedKey,
    onMutate: async (asked) => {
      // A refetch already on its way would answer with the state from before.
      await queryClient.cancelQueries({ queryKey: browseKey });
      showMarked(queryClient, asked);
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: browseKey });
    },
  });
  return mutate;
}

/** What `asked` marks, with its Library, or nothing when it doesn't say, which the Server refuses. */
function marking({ libraryId, target, played }: Asked) {
  return libraryId === undefined || target?.case === undefined
    ? undefined
    : { libraryId, target, played: played === true };
}

/** Marks what `asked` names in the cached Movie or Episodes, before the Server has answered. */
function showMarked(queryClient: QueryClient, asked: Asked) {
  const mark = marking(asked);
  if (!mark) {
    return;
  }
  const { libraryId, target, played } = mark;
  if (target.case === "movieId") {
    showMovieMarked(queryClient, { libraryId, movieId: target.value }, played);
    return;
  }
  showEpisodesMarked(queryClient, { libraryId, target, played });
}

/** Marks the cached Episodes `mark` covers: one Episode, or a Season's or Show's. */
function showEpisodesMarked(
  queryClient: QueryClient,
  {
    libraryId,
    target,
    played,
  }: { libraryId: bigint; target: Exclude<Target, { case: "movieId" }>; played: boolean },
) {
  const key = createConnectQueryKey({
    schema: BrowseService.method.listEpisodes,
    cardinality: undefined,
    input: episodesAsked(libraryId, target),
  });
  const one = target.case === "episodeId" ? target.value : undefined;
  queryClient.setQueriesData<ListEpisodesResponse>(
    { queryKey: key },
    (data) => data && markEpisodes(data, played, one),
  );
}

/** Marks the cached Movie `asked` names. */
function showMovieMarked(
  queryClient: QueryClient,
  asked: { libraryId: bigint; movieId: MessageInitShape<typeof ExternalIdSchema> },
  played: boolean,
) {
  const key = createConnectQueryKey({
    schema: BrowseService.method.getMovie,
    cardinality: undefined,
    input: asked,
  });
  queryClient.setQueriesData<GetMovieResponse>(
    { queryKey: key },
    (data) => data && markMovie(data, asked.movieId, played),
  );
}

/**
 * The Episodes lists a mark of `target` covers, as their queries ask for them: an Episode's is one
 * of its Library's lists, a Season's or Show's the whole of theirs.
 */
function episodesAsked(
  libraryId: bigint,
  target: Exclude<Target, { case: "movieId" }>,
): MessageInitShape<typeof ListEpisodesRequestSchema> {
  if (target.case === "season") {
    // Without its Show the Server refuses it, and `{}` would match every Show's list.
    const { showId = { namespace: "-" }, seasonNumber = 0 } = target.value;
    return { libraryId, showId, seasonNumber };
  }
  return target.case === "showId" ? { libraryId, showId: target.value } : { libraryId };
}
