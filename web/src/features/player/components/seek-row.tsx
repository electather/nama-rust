import { useState } from "react";

import { SeekPreview } from "@/features/player/components/seek-preview";
import type { Video } from "@/features/player/hooks/use-video";
import { seekBarShortcutFor } from "@/features/player/lib/seek-bar-keys";
import { type SeekBar, sectionAt } from "@/features/player/lib/seek-sections";
import { clock } from "@/shared/lib/clock";
import { Slider } from "@/shared/ui/slider";

const HOUR_SECONDS = 3600;

function clockFit(duration: number) {
  const long = duration >= HOUR_SECONDS;
  return { long, width: long ? "min-w-17" : "min-w-12" };
}

/**
 * Where it's playing: the time played, the seek bar, and the time left. A drag holds the bar and
 * the times where the pointer is and seeks once, when it ends: following the `<video>` instead, the
 * thumb would wait on each seek and jump back to the time it last reported. The bar is split into
 * the `seekBar`'s sections, and its hover card shows the frame, section and time under the pointer.
 */
export function SeekRow({
  video,
  seekBar: { sections, sheets },
}: {
  video: Video;
  seekBar: SeekBar;
}) {
  const { time, duration, buffered } = video.state;
  const [held, setHeld] = useState<number>();
  const at = held ?? time;
  const { long, width } = clockFit(duration);
  return (
    <div
      className="flex items-center gap-2.5 text-sm tabular-nums"
      // Left and Right, alone or with Ctrl/⌥, do what the player's keys do, not step the slider
      // 1 s, each step its own seek that Remux plays wait on. Prevented, so the slider skips them
      // (Base UI checks) and so does the player's own key handler.
      onKeyDownCapture={(event) => {
        const shortcut = seekBarShortcutFor(event.nativeEvent);
        if (shortcut) {
          event.preventDefault();
          video.control((element) => {
            shortcut({ video: element, sections });
          });
        }
      }}
    >
      <span className={width}>{clock(at, long)}</span>
      <Slider
        label="Position"
        valueText={`${clock(at)} of ${clock(duration)}`}
        disabled={duration === 0}
        value={Math.min(at, duration)}
        max={duration === 0 ? 1 : duration}
        buffered={buffered}
        sections={sections.length > 0 ? sections.map(({ start }) => start / duration) : undefined}
        renderPoint={({ fraction, width: barWidth }) => (
          <SeekPreview
            fraction={fraction}
            width={barWidth}
            seconds={fraction * duration}
            name={sectionAt(sections, fraction * duration)}
            sheets={sheets}
          />
        )}
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
