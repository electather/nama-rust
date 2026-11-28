import { Link } from "@tanstack/react-router";

import { type ShowInLibrary, showParams } from "@/features/title/lib/show-params";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import { backdropLink } from "@/shared/lib/backdrop-card-classes";

/** What covers an Episode's card of `show` and plays its largest Version, named `label`. */
export function EpisodePlayLink({
  show,
  episode,
  label,
}: {
  show: ShowInLibrary;
  episode: Episode;
  label: string;
}) {
  return (
    <Link
      to="/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play"
      params={{
        ...showParams(show),
        season: String(episode.seasonNumber),
        episode: String(episode.episodeNumber),
      }}
      aria-label={label}
      className={backdropLink}
    />
  );
}
