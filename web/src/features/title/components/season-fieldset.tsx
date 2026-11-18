import { SeasonChoices } from "@/features/title/components/season-choices";
import { WholeShowChoice } from "@/features/title/components/whole-show-choice";
import type { FoundSeason, FoundShow } from "@/gen/nama/v1/request_pb";

/** The Whole Show and each of `show`'s Seasons, as choices, those that are `pickable` enabled. */
export function SeasonFieldset({
  show,
  wholePickable,
  pickable,
  onWholeChange,
}: {
  show: FoundShow;
  wholePickable: boolean;
  pickable: (season: FoundSeason) => boolean;
  onWholeChange: (whole: boolean) => void;
}) {
  return (
    <fieldset>
      <legend className="font-medium">Seasons</legend>
      <ul className="mt-3 flex flex-col">
        <WholeShowChoice
          followedShow={show.followedShow}
          disabled={!wholePickable}
          onChange={onWholeChange}
        />
        <SeasonChoices show={show} pickable={pickable} />
      </ul>
    </fieldset>
  );
}
