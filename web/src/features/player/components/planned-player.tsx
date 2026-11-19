import { CantPlay } from "@/features/player/components/cant-play";
import { DroppedChoice } from "@/features/player/components/dropped-choice";
import { PlaybackInfo } from "@/features/player/components/playback-info";
import { TopBar } from "@/features/player/components/top-bar";
import { VideoPlayer } from "@/features/player/components/video-player";
import { useSubtitleAppearance } from "@/features/player/data/use-subtitle-appearance";
import { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import { wordReasons } from "@/features/player/lib/playback-wording";
import type { PlannedProps } from "@/features/player/lib/player-props";

/** Plays the Version with the plan the Server gives, or says it can't. */
export function PlannedPlayer(props: PlannedProps) {
  const planned = usePlaybackPlan(props);
  const subtitles = useSubtitleAppearance();
  const { plan } = planned;
  if (plan.url === "") {
    return <CantPlay back={props.back} reasons={wordReasons(plan.reasons).decided} />;
  }
  if (planned.gaveUp) {
    return <CantPlay back={props.back} reasons={[]} />;
  }
  const topBar = (
    <TopBar
      title={props.title}
      back={props.back}
      playbackInfo={<PlaybackInfo plan={plan} />}
      notice={
        <>
          {props.note}
          <DroppedChoice justChose={planned.justChose} plan={plan} />
        </>
      }
    />
  );
  return (
    // `subtitle-appearance.css` styles the text subtitles' cues by these.
    <div data-subtitle-size={subtitles.size} data-subtitle-background={subtitles.background}>
      {/* Each new plan plays in a new `<video>`, so it starts where that plan does. ponytail: that
          one starts unmuted at full volume; carry those over if anyone minds. */}
      <VideoPlayer key={planned.asks} {...props} planned={planned} topBar={topBar} />
    </div>
  );
}
