import { CantPlay } from "@/features/player/components/cant-play";
import { PlayingPlan } from "@/features/player/components/playing-plan";
import { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { PlannedProps } from "@/features/player/lib/player-props";

/** Plays the Version with the plan the Server gives, or says it can't. */
export function PlannedPlayer(props: PlannedProps) {
  const planned = usePlaybackPlan(props);
  const { plan } = planned;
  if (plan.url === "") {
    return <CantPlay {...props} reasons={plan.reasons.filter(({ decided }) => decided)} />;
  }
  if (planned.gaveUp) {
    return <CantPlay {...props} reasons={[]} />;
  }
  return <PlayingPlan {...props} planned={planned} />;
}
