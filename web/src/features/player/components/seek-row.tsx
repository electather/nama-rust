import { useState } from "react";

import type { Video } from "@/features/player/hooks/use-video";
import { clock } from "@/features/player/lib/video-actions";
import { Slider } from "@/shared/ui/slider";

const HOUR_SECONDS = 3600;

/** Whether the times show hours, and the width that keeps them from shifting the seek bar. */
function clockFit(duration: number) {
  const long = duration >= HOUR_SECONDS;
  return { long, width: long ? "min-w-17" : "min-w-12" };
}

/**
 * Where it's playing: the time played, the seek bar, and the time left. A drag holds the bar and
 * the times where the pointer is and seeks once, when it ends: following the `<video>` instead, the
 * thumb would wait on each seek and jump back to the time it last reported.
 */
export function SeekRow({ video }: { video: Video }) {
  const { time, duration, buffered } = video.state;
  const [held, setHeld] = useState<number>();
  const at = held ?? time;
  const { long, width } = clockFit(duration);
  return (
    <div className="flex items-center gap-2.5 text-sm tabular-nums">
      <span className={width}>{clock(at, long)}</span>
      <Slider
        label="Position"
        valueText={`${clock(at)} of ${clock(duration)}`}
        disabled={duration === 0}
        value={Math.min(at, duration)}
        max={duration === 0 ? 1 : duration}
        buffered={buffered}
        pointText={(fraction) => clock(fraction * duration)}
        onValueChange={setHeld}
        onValueCommitted={(seconds) => {
          video.control((element) => {
            element.currentTime = seconds;
          });
          setHeld(undefined);
        }}
      />
      <span className={`${width} text-end`}>−{clock(Math.max(0, duration - at))}</span>
    </div>
  );
}
