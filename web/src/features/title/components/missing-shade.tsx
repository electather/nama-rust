import { EpisodeCorner } from "@/features/title/components/episode-corner";
import type { Request } from "@/gen/nama/v1/request_pb";

/**
 * What dims a missing Episode's Still, greyed and darkened toward the page, with where its Season
 * `season`'s `request` is in its top corner, while it has one or is `askable`.
 */
export function MissingShade({
  season,
  request,
  askable,
}: {
  season: number;
  request: Request | undefined;
  askable: boolean;
}) {
  return (
    <>
      <div aria-hidden className="absolute inset-0 bg-background/55 backdrop-grayscale" />
      {(askable || request) && <EpisodeCorner season={season} request={request} />}
    </>
  );
}
