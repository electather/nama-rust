import { CantPlay } from "@/features/player/components/cant-play";
import { DroppedChoice } from "@/features/player/components/dropped-choice";
import { VideoPlayer } from "@/features/player/components/video-player";
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
  const topBar = {
    title: props.heading ?? props.title,
    line: props.line,
    back: props.back,
    notice: (
      <>
        {props.note}
        <DroppedChoice justChose={planned.justChose} plan={plan} />
      </>
    ),
  };
  return (
    // Each new plan plays in a new `<video>`, so it starts where that plan does. ponytail: that one
    // starts unmuted at full volume; carry those over if anyone minds.
    <VideoPlayer key={planned.asks} {...props} planned={planned} topBar={topBar} />
  );
}
