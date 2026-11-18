import type { Movie, Season, Show } from "@/gen/nama/v1/browse_pb";
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import type { EditableImage } from "@/shared/lib/editable-image";
import { seasonName } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** A title's images, for Edit images. */
export interface TitleImages {
  images: EditableImage[];
}

/** A Movie's Poster and Backdrop, for Edit images. */
export function movieImages(movieId: ExternalId, movie: Movie): TitleImages {
  const path = `/custom-images/movie/${titleParam(movieId)}`;
  const asked = { kind: TitleKind.MOVIE, id: movieId };
  const images: EditableImage[] = [
    {
      name: "Poster",
      url: movie.posterUrl,
      custom: movie.customPoster,
      path: `${path}/poster`,
      shape: "poster",
      artwork: { asked, list: "posterUrls" },
    },
    {
      name: "Backdrop",
      url: movie.backdropUrl,
      custom: movie.customBackdrop,
      path: `${path}/backdrop`,
      shape: "backdrop",
      artwork: { asked, list: "backdropUrls" },
    },
  ];
  return { images };
}

/** A Show's Poster and Backdrop, and the Poster of its Season `season`, for Edit images. */
export function showImages(showId: ExternalId, show: Show, season?: Season): TitleImages {
  const path = `/custom-images/show/${titleParam(showId)}`;
  const asked = { kind: TitleKind.SHOW, id: showId };
  const images: EditableImage[] = [
    {
      name: "Poster",
      url: show.posterUrl,
      custom: show.customPoster,
      path: `${path}/poster`,
      shape: "poster",
      artwork: { asked, list: "posterUrls" },
    },
  ];
  if (season) {
    images.push({
      name: `${seasonName(season.number)} Poster`,
      url: season.posterUrl,
      custom: season.customPoster,
      path: `${path}/season/${String(season.number)}/poster`,
      shape: "poster",
      artwork: { asked: { ...asked, season: season.number }, list: "posterUrls" },
    });
  }
  images.push({
    name: "Backdrop",
    url: show.backdropUrl,
    custom: show.customBackdrop,
    path: `${path}/backdrop`,
    shape: "backdrop",
    artwork: { asked, list: "backdropUrls" },
  });
  return { images };
}
