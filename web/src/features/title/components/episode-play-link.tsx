import { Link, useParams } from "@tanstack/react-router";

import type { Episode } from "@/gen/nama/v1/browse_pb";
import { backdropLink } from "@/shared/lib/backdrop-card-classes";

/** What covers a Library Episode's card and plays its largest Version, named `label`. */
export function EpisodePlayLink({ episode, label }: { episode: Episode; label: string }) {
  const show = useParams({ from: "/_member/_server/libraries/$libraryId/shows/$showId/" });
  return (
    <Link
      to="/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play"
      params={{
        ...show,
        season: String(episode.seasonNumber),
        episode: String(episode.episodeNumber),
      }}
      aria-label={label}
      className={backdropLink}
    />
  );
}
