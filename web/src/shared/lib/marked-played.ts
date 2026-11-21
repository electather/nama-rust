import { type MessageInitShape, create } from "@bufbuild/protobuf";

import type { GetMovieResponse, ListEpisodesResponse } from "@/gen/nama/v1/browse_pb";
import type { ExternalIdSchema } from "@/gen/nama/v1/match_pb";
import { type WatchState, WatchStateSchema } from "@/gen/nama/v1/watch_state_pb";

/** An External ID as a mark asks for one or a title carries it. */
export type Id = MessageInitShape<typeof ExternalIdSchema>;

/**
 * `state` once marked `played` or unplayed, as the Server will answer it: either way it starts
 * over, so no bar of how far in shows.
 */
function marked(state: WatchState | undefined, played: boolean): WatchState {
  const { durationMs, updatedAt } = state ?? create(WatchStateSchema);
  return create(WatchStateSchema, { durationMs, updatedAt, played, positionMs: 0n });
}

function same(a: Id = {}, b: Id = {}) {
  return a.namespace === b.namespace && a.value === b.value;
}

/** `data` with the Movie `id` marked `played`, or as it was if it's another Movie. */
export function markMovie(data: GetMovieResponse, id: Id, played: boolean): GetMovieResponse {
  const { movie } = data;
  if (!movie || !same(movie.id, id)) {
    return data;
  }
  return { ...data, movie: { ...movie, watchState: marked(movie.watchState, played) } };
}

/** `data` with its Episodes marked `played`: only the one `id` names, or every one without it. */
export function markEpisodes(
  data: ListEpisodesResponse,
  played: boolean,
  id?: Id,
): ListEpisodesResponse {
  return {
    ...data,
    episodes: data.episodes.map((episode) =>
      id === undefined || same(episode.id, id)
        ? { ...episode, watchState: marked(episode.watchState, played) }
        : episode,
    ),
  };
}
