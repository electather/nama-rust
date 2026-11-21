import type { StillArt } from "@/features/action-bar/lib/rows";
import type { MovieHit, Request, ShowHit } from "@/gen/nama/v1/request_pb";
import type { EpisodeHit, SearchResponse } from "@/gen/nama/v1/search_pb";
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
  /** The Library it opens in, or none for a Movie to request, which opens its request page. */
  libraryId: string | undefined;
  /** The open Request the User can see for a Movie to request, which has no `libraryId`. */
  request?: Request | undefined;
  /** The Movie's or Show's id as a URL writes it. An Episode's is its Show's. */
  titleId: string;
  /** The Season an Episode's Show opens at. */
  season?: number;
  label: string;
  hint: string;
  /** A Movie's or Show's Poster, or an Episode's Show's. */
  poster?: string | undefined;
  /** An Episode's Still. */
  still?: StillArt | undefined;
}

/** What a search found, by kind, each in the order to list it. */
export interface Found {
  movies: FoundTitle[];
  shows: FoundTitle[];
  episodes: FoundTitle[];
  /** Whether the titles to request couldn't be searched, so only this Server's are found. */
  titleSourceUnavailable: boolean;
}

function movieFound(movie: MovieHit): FoundTitle {
  const titleId = titleParam(movie.id);
  const libraryId = movie.libraryId?.toString();
  return {
    id: `movie:${libraryId ?? "to-request"}:${titleId}`,
    kind: "movie",
    libraryId,
    // Only a Movie to request shows its Request; one in a Library stays plain data for Recent.
    request: libraryId === undefined ? movie.request : undefined,
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
    poster: episode.showPosterUrl,
    still:
      episode.stillUrl === undefined
        ? undefined
        : { url: episode.stillUrl, thumbhash: episode.stillThumbhash },
  };
}

/**
 * What `Search` has streamed so far, as the action bar lists it: each kind by status (Available,
 * Requested, then Not on this Server, as their numbers go), and in the Server's order within one.
 */
export function foundIn(responses: readonly SearchResponse[]): Found {
  const titles = responses
    .flatMap(({ part }) => (part.case === "hits" ? part.value.hits : []))
    // oxlint-disable-next-line unicorn/no-array-sort -- sorts the array just built, and the lib target has no toSorted
    .sort((a, b) => a.status - b.status)
    .map(({ title }) => title);
  return {
    movies: titles.flatMap((title) => (title.case === "movie" ? [movieFound(title.value)] : [])),
    shows: titles.flatMap((title) => (title.case === "show" ? [showFound(title.value)] : [])),
    episodes: titles.flatMap((title) =>
      title.case === "episode" ? [episodeFound(title.value)] : [],
    ),
    titleSourceUnavailable: responses.some(
      ({ part }) => part.case === "done" && part.value.titleSourceUnavailable,
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
