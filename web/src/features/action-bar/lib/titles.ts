import type { MovieHit, ShowHit } from "@/gen/nama/v1/request_pb";
import type { EpisodeHit, SearchResponse } from "@/gen/nama/v1/search_pb";
import { metadataLine, seasonName } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/**
 * A Movie, Show or Episode in a Library, as the action bar lists it, opens it and remembers it.
 * Plain data, so Recent can keep it in `localStorage`.
 */
export interface FoundTitle {
  /** Unique among titles: the same title in two Libraries is two of these. */
  id: string;
  kind: "movie" | "show" | "episode";
  libraryId: string;
  /** The Movie's or Show's id as a URL writes it. An Episode's is its Show's. */
  titleId: string;
  /** The Season an Episode's Show opens at. */
  season?: number;
  label: string;
  hint: string;
  poster?: string | undefined;
}

/** What a search found, by kind, each best fit first. */
export interface Found {
  movies: FoundTitle[];
  shows: FoundTitle[];
  episodes: FoundTitle[];
}

function movieFound(movie: MovieHit): FoundTitle {
  const titleId = titleParam(movie.id);
  return {
    id: `movie:${movie.libraryId}:${titleId}`,
    kind: "movie",
    libraryId: String(movie.libraryId),
    titleId,
    label: movie.title,
    hint: metadataLine(movie.year?.toString()),
    poster: movie.posterUrl,
  };
}

function showFound(show: ShowHit): FoundTitle {
  const titleId = titleParam(show.id);
  return {
    id: `show:${show.libraryId}:${titleId}`,
    kind: "show",
    libraryId: String(show.libraryId),
    titleId,
    label: show.title,
    hint: "",
    poster: show.posterUrl,
  };
}

function episodeFound(episode: EpisodeHit): FoundTitle {
  return {
    id: `episode:${episode.libraryId}:${titleParam(episode.id)}`,
    kind: "episode",
    libraryId: String(episode.libraryId),
    titleId: titleParam(episode.showId),
    season: episode.seasonNumber,
    label: `${episode.episodeNumber}. ${episode.title}`,
    hint: metadataLine(episode.showTitle, seasonName(episode.seasonNumber)),
  };
}

/** What `Search` has streamed so far, as the action bar lists it, each kind in the Server's order. */
export function foundIn(responses: readonly SearchResponse[]): Found {
  const titles = responses
    .flatMap(({ part }) => (part.case === "hits" ? part.value.hits : []))
    .map(({ title }) => title);
  return {
    movies: titles.flatMap((title) => (title.case === "movie" ? [movieFound(title.value)] : [])),
    shows: titles.flatMap((title) => (title.case === "show" ? [showFound(title.value)] : [])),
    episodes: titles.flatMap((title) =>
      title.case === "episode" ? [episodeFound(title.value)] : [],
    ),
  };
}

const kinds = new Set(["movie", "show", "episode"]);
const texts = ["id", "kind", "libraryId", "titleId", "label", "hint"];

/** Whether `value`, read back from storage, is still a title this version can list and open. */
export function isFoundTitle(value: unknown): value is FoundTitle {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const field = (name: string): unknown => Reflect.get(value, name);
  return texts.every((name) => typeof field(name) === "string") && kinds.has(String(field("kind")));
}
