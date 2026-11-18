import { Choice, type ChoiceProps } from "@/features/title/components/choice";
import type { FollowedShow } from "@/gen/nama/v1/request_pb";
import { RequestStateLabel } from "@/shared/components/request-state/request-state-label";

/** The Whole Show as a Choice, with the state of its open Followed Show, if it has one. */
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
      {followedShow && <RequestStateLabel state={followedShow.state} />}
    </Choice>
  );
}
