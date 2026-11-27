import type { MovieHit, Request, ShowHit } from "@/gen/nama/v1/request_pb";
import type { EpisodeHit, SearchResponse, TitleStatus } from "@/gen/nama/v1/search_pb";
import { hitsIn, searchNote, type StreamedHit } from "@/shared/lib/search-hits";
import { type ServerImage, headshotOf, posterOf, stillOf } from "@/shared/lib/server-image";
import { metadataLine, seasonName } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/**
 * A Movie, Show or Episode, as the action bar lists it, opens it and remembers it. One in a Library
 * is plain data, so Recent can keep it in `localStorage`, all but its images' thumbhashes.
 */
export interface FoundTitle {
  /** Unique among titles: the same title in two Libraries is two of these. */
  id: string;
  kind: "movie" | "show" | "episode";
  /** At the Hub, the Linked Server it was found on, which it opens on. */
  server?: string | undefined;
  /** The Library it opens in, or none for a title to request, which opens its request page. */
  libraryId: string | undefined;
  /** The open Request the User can see for a title to request, which has no `libraryId`. */
  request?: Request | undefined;
  /** How close a search found it to being on the Server. Recent doesn't keep it. */
  status?: TitleStatus;
  /** Whether the title source's batch held it, not a Search source's. Recent doesn't keep it. */
  titleSource?: boolean;
  /** The Movie's or Show's id as a URL writes it. An Episode's is its Show's. */
  titleId: string;
  /** The Season an Episode's Show opens at. */
  season?: number;
  label: string;
  hint: string;
  /** A Movie's or Show's Poster, or an Episode's Show's. */
  poster?: ServerImage | undefined;
  /** An Episode's Still. */
  still?: ServerImage | undefined;
}

/** A Person a search found, as the action bar lists them and opens their page. */
export interface FoundPerson {
  /** Their id as a URL writes it. */
  personId: string;
  /** At the Hub, the Linked Server they were found on. */
  server?: string | undefined;
  name: string;
  headshot: ServerImage | undefined;
}

/** What a search found, each kind in the order to list it. */
export interface Found {
  /** The Movies and Shows in one list, as the root list mixes them in the Server's order. */
  titles: FoundTitle[];
  episodes: FoundTitle[];
  people: FoundPerson[];
  /** What is missing from the results, as `searchNote` says it, or "". */
  note: string;
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
    poster: posterOf(movie),
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
    poster: posterOf(show),
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
    poster: posterOf({
      posterUrl: episode.showPosterUrl,
      posterThumbhash: episode.showPosterThumbhash,
    }),
    still: stillOf(episode),
  };
}

/** A title one Server's `Search` found, and at the Hub, which Linked Server that is. */
interface ServerHit extends StreamedHit {
  server?: string | undefined;
}

/** What one Server's `Search` has streamed so far, and at the Hub, which Linked Server it is. */
export interface SearchAnswer {
  server?: string | undefined;
  responses: readonly SearchResponse[];
}

/**
 * What `Search` has streamed so far, as the action bar lists it, in `hitsIn`'s order. At the Hub,
 * it's every Server's answer as one list: each title once, as the Server that has it closest to
 * hand has it, the first in `answers` on a tie.
 */
export function foundIn(answers: readonly SearchAnswer[]): Found {
  const isNew = firstOfEach();
  const hits = answers
    .flatMap(({ server, responses }) =>
      hitsIn(responses).map(({ hit, titleSource }) => ({ hit, titleSource, server })),
    )
    // oxlint-disable-next-line unicorn/no-array-sort -- sorts the array just built, and the lib target has no toSorted; it's stable, so Servers keep their order within a status
    .sort((a, b) => a.hit.status - b.hit.status)
    // On a Server's own web app, a title in two Libraries is listed in both.
    .filter(
      ({ hit: { title }, server }) =>
        server === undefined || isNew(`${title.case}:${titleParam(title.value?.id)}`),
    );
  const notes = new Set(answers.map(({ responses }) => searchNote(responses)));
  return foundOf(hits, [...notes].filter(Boolean).join(" "));
}

/** Says whether each key it's given is one it hasn't been given before. */
function firstOfEach() {
  const seen = new Set<string>();
  return (key: string) => {
    const first = !seen.has(key);
    seen.add(key);
    return first;
  };
}

/** The titles `hits` lists, as the action bar lists them, and `note` on what's missing. */
function foundOf(hits: readonly ServerHit[], note: string): Found {
  return {
    titles: hits.flatMap(({ hit: { title, status }, server, titleSource }) =>
      [
        ...(title.case === "movie" ? [movieFound(title.value, status)] : []),
        ...(title.case === "show" ? [showFound(title.value, status)] : []),
      ].map((found) => foundOn({ ...found, titleSource }, server)),
    ),
    episodes: hits.flatMap(({ hit: { title }, server }) =>
      title.case === "episode" ? [foundOn(episodeFound(title.value), server)] : [],
    ),
    people: hits.flatMap(({ hit: { title }, server }) =>
      title.case === "person"
        ? [
            {
              personId: titleParam(title.value.id),
              server,
              name: title.value.name,
              headshot: headshotOf(title.value),
            },
          ]
        : [],
    ),
    note,
  };
}

/** `title`, found at the Hub on the Linked Server `server`, or as it is on a Server's own. */
function foundOn(title: FoundTitle, server: string | undefined): FoundTitle {
  return server === undefined ? title : { ...title, id: `${server}:${title.id}`, server };
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
