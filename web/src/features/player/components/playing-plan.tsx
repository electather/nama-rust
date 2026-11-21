import { DroppedChoice } from "@/features/player/components/dropped-choice";
import { VideoPlayer } from "@/features/player/components/video-player";
import { useSubtitleAppearance } from "@/features/player/data/use-subtitle-appearance";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { PlannedProps } from "@/features/player/lib/player-props";

/** The `<video>` of a plan that plays, under the title bar, in the User’s Subtitle Appearance. */
export function PlayingPlan({
  planned,
  ...props
}: PlannedProps & { planned: ReturnType<typeof usePlaybackPlan> }) {
  const subtitles = useSubtitleAppearance();
  const topBar = {
    title: props.heading ?? props.title,
    line: props.line,
    back: props.back,
    notice: (
      <>
        {props.note}
        <DroppedChoice justChose={planned.justChose} plan={planned.plan} />
      </>
    ),
  };
  return (
    // `src/subtitle-cues.css` styles the text subtitles' cues by these.
    <div data-subtitle-size={subtitles.size} data-subtitle-background={subtitles.background}>
      {/* Each new plan plays in a new `<video>`, so it starts where that plan does. ponytail: that
          one starts unmuted at full volume; carry those over if anyone minds. */}
      <VideoPlayer key={planned.asks} {...props} planned={planned} topBar={topBar} />
    </div>
  );
}
