import { Choice } from "@/features/title/components/choice";
import { SeasonState } from "@/features/title/components/season-state";
import type { FoundSeason, FoundShow } from "@/gen/nama/v1/request_pb";
import { RequestProgress } from "@/shared/components/request-progress/request-progress";
import { seasonName, yearOf } from "@/shared/lib/title-facts";

/** Each of `show`'s Seasons as a Choice, which can be picked if it's `pickable`. */
export function SeasonChoices({
  show,
  pickable,
}: {
  show: FoundShow;
  pickable: (season: FoundSeason) => boolean;
}) {
  if (show.seasons.length === 0) {
    return <li className="py-2 text-muted-foreground">{show.title} has no Seasons yet.</li>;
  }
  return show.seasons.map((season) => (
    <Choice
      key={season.seasonNumber}
      name="season"
      value={season.seasonNumber}
      label={seasonName(season.seasonNumber)}
      detail={yearOf(season.airDate)}
      disabled={!pickable(season)}
      below={<RequestProgress request={season.request} />}
    >
      <SeasonState season={season} />
    </Choice>
  ));
}
