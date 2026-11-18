import { linkOptions } from "@tanstack/react-router";

import type {
  EpisodeInLibrary,
  MovieInLibrary,
  SearchLibrariesResponse,
  ShowInLibrary,
} from "@/gen/nama/v1/browse_pb";
import { episodeName, metadataLine, yearOf } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

const movieLink = (movie: MovieInLibrary) =>
  linkOptions({
    to: "/libraries/$libraryId/movies/$movieId",
    params: { libraryId: String(movie.libraryId), movieId: titleParam(movie.id) },
  });

const showLink = (libraryId: bigint, showId: ShowInLibrary["id"], season?: number) =>
  linkOptions({
    to: "/libraries/$libraryId/shows/$showId",
    params: { libraryId: String(libraryId), showId: titleParam(showId) },
    search: season === undefined ? {} : { season },
  });

/** A Movie, Show or Episode found in the User's Libraries, and where it opens. */
export interface FoundTitle {
  /** Names the Library too: a title in two Libraries is found in each. */
  key: string;
  kind: "movie" | "show" | "episode";
  title: string;
  hint: string;
  posterUrl: string | undefined;
  link: ReturnType<typeof movieLink> | ReturnType<typeof showLink>;
}

function episodeFound(episode: EpisodeInLibrary): FoundTitle {
  return {
    key: `episode/${episode.libraryId}/${titleParam(episode.id)}`,
    kind: "episode",
    title: episode.title,
    hint: metadataLine(
      episode.showTitle,
      episodeName(episode.seasonNumber, episode.episodeNumber, "short"),
    ),
    posterUrl: undefined,
    link: showLink(episode.libraryId, episode.showId, episode.seasonNumber),
  };
}

/** What a search found, Movies then Shows then Episodes, each opening its page. */
export function foundTitles({ movies, shows, episodes }: SearchLibrariesResponse): FoundTitle[] {
  return [
    ...movies.map((movie) => ({
      key: `movie/${movie.libraryId}/${titleParam(movie.id)}`,
      kind: "movie" as const,
      title: movie.title,
      hint: yearOf(movie.releaseDate) ?? "",
      posterUrl: movie.posterUrl,
      link: movieLink(movie),
    })),
    ...shows.map((show) => ({
      key: `show/${show.libraryId}/${titleParam(show.id)}`,
      kind: "show" as const,
      title: show.title,
      hint: "",
      posterUrl: show.posterUrl,
      link: showLink(show.libraryId, show.id),
    })),
    ...episodes.map((episode) => episodeFound(episode)),
  ];
}
