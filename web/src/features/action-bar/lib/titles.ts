import type { MovieHit, Request, ShowHit } from "@/gen/nama/v1/request_pb";
import type { EpisodeHit, SearchResponse, TitleStatus } from "@/gen/nama/v1/search_pb";
import { hitsIn, titleSourceUnavailable } from "@/shared/lib/search-hits";
import { metadataLine, seasonName } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/**
 * A Movie, Show or Episode, as the action bar lists it, opens it and remembers it. One in a Library
 * is plain data, so Recent can keep it in `localStorage`.
 */
export interface FoundTitle {
  /** Unique among titles: the same title in two Libraries is two of these. */
  id: string;
  kind: "movie" | "show" | "episode";
  /** The Library it opens in, or none for a title to request, which opens its request page. */
  libraryId: string | undefined;
  /** The open Request the User can see for a title to request, which has no `libraryId`. */
  request?: Request | undefined;
  /** How close a search found it to being on the Server. Recent doesn't keep it. */
  status?: TitleStatus;
  /** The Movie's or Show's id as a URL writes it. An Episode's is its Show's. */
  titleId: string;
  /** The Season an Episode's Show opens at. */
  season?: number;
  label: string;
  hint: string;
  poster?: string | undefined;
}

/** What a search found, by kind, each in the order to list it. */
export interface Found {
  movies: FoundTitle[];
  shows: FoundTitle[];
  episodes: FoundTitle[];
  /** Whether the titles to request couldn't be searched, so only this Server's are found. */
  titleSourceUnavailable: boolean;
}

function movieFound(movie: MovieHit, status: TitleStatus): FoundTitle {
  const titleId = titleParam(movie.id);
  const libraryId = movie.libraryId?.toString();
  return {
    id: `movie:${libraryId ?? "to-request"}:${titleId}`,
    kind: "movie",
    libraryId,
    // Only a title to request shows its Request; one in a Library stays plain data for Recent.
    request: libraryId === undefined ? movie.request : undefined,
    status,
    titleId,
    label: movie.title,
    hint: metadataLine(movie.year?.toString()),
    poster: movie.posterUrl,
  };
}

function showFound(show: ShowHit, status: TitleStatus): FoundTitle {
  const titleId = titleParam(show.id);
  const libraryId = show.libraryId?.toString();
  return {
    id: `show:${libraryId ?? "to-request"}:${titleId}`,
    kind: "show",
    libraryId,
    request: libraryId === undefined ? show.request : undefined,
    status,
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

/** What `Search` has streamed so far, as the action bar lists it: each kind in `hitsIn`'s order. */
export function foundIn(responses: readonly SearchResponse[]): Found {
  const titles = hitsIn(responses);
  return {
    movies: titles.flatMap(({ title, status }) =>
      title.case === "movie" ? [movieFound(title.value, status)] : [],
    ),
    shows: titles.flatMap(({ title, status }) =>
      title.case === "show" ? [showFound(title.value, status)] : [],
    ),
    episodes: titles.flatMap(({ title }) =>
      title.case === "episode" ? [episodeFound(title.value)] : [],
    ),
    titleSourceUnavailable: titleSourceUnavailable(responses),
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
