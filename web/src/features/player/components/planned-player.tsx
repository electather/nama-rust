import { CantPlay } from "@/features/player/components/cant-play";
import { PlayingPlan } from "@/features/player/components/playing-plan";
import { StoppedByAdmin } from "@/features/player/components/stopped-by-admin";
import { useAdminStop } from "@/features/player/data/use-admin-stop";
import { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { PlannedProps } from "@/features/player/lib/player-props";

/** Plays the Version with the plan the Server gives, or says it can't, or that an Admin stopped it. */
export function PlannedPlayer(props: PlannedProps) {
  const planned = usePlaybackPlan(props);
  const { plan } = planned;
  const adminStop = useAdminStop(plan.id);
  if (adminStop) {
    return <StoppedByAdmin message={adminStop.message} back={props.back} />;
  }
  if (plan.url === "") {
    return <CantPlay {...props} reasons={plan.reasons.filter(({ decided }) => decided)} />;
  }
  if (planned.gaveUp) {
    return <CantPlay {...props} reasons={[]} />;
  }
  return <PlayingPlan {...props} planned={planned} />;
}
