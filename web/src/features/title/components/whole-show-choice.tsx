import { Choice, type ChoiceProps } from "@/features/title/components/choice";
import type { FollowedShow } from "@/gen/nama/v1/request_pb";
import { shownOnTitle } from "@/shared/components/request-state/request-states";
import { StateLabel } from "@/shared/components/state-label";

/**
 * The Whole Show as a Choice, with the state of its open Followed Show, if it has one. A plain
 * pill: a Followed Show has no downloads or failure of its own, its Seasons do.
 */
export function WholeShowChoice({
  followedShow,
  ...choice
}: { followedShow: FollowedShow | undefined } & Pick<ChoiceProps, "disabled" | "onChange">) {
  return (
    <Choice
      name="whole"
      label="Whole Show"
      description="Every numbered Season, and new ones as they come out"
      {...choice}
    >
      {followedShow && <StateLabel {...shownOnTitle(followedShow.state)} />}
    </Choice>
  );
}
