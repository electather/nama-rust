import { Button } from "nama-ui/components/button";
import { useState } from "react";

import type { Video } from "@/features/player/hooks/use-video";
import { skippable } from "@/features/player/lib/segments";
import type { Segment } from "@/gen/nama/v1/playback_pb";

/** "Skip intro" or "Skip recap" while one plays, which seeks to where it ends. Hidden otherwise. */
export function SkipSegmentButton({ segments, video }: { segments: Segment[]; video: Video }) {
  const skip = skippable(segments, video.state);
  // Kept after the Segment ends, so the button still says it while it fades out.
  const [label, setLabel] = useState("");
  if (skip && skip.label !== label) {
    setLabel(skip.label);
  }
  return (
    // Placed as the Next Episode card is: outside the controls, so it stays while they hide. Always
    // there, so `hidden` can fade it out: `allow-discrete` holds off `display: none` until the fade
    // ends, where the browser can, and `@starting-style` fades it in, as the artboard's `pl-in` does.
    <div
      hidden={skip === undefined}
      className="absolute inset-e-5 bottom-26 z-30 transition-all transition-discrete duration-200 ease-out-expo group-data-controls-hidden:bottom-5 motion-reduce:transition-none starting:opacity-0 motion-safe:starting:translate-y-1.5 gone:opacity-0 motion-safe:gone:translate-y-1.5"
    >
      <Button
        size="tall"
        onClick={() => {
          video.control((element) => {
            // Still there while it fades out, after the Segment it skipped ended.
            element.currentTime = skip?.end ?? element.currentTime;
          });
        }}
      >
        {label}
      </Button>
    </div>
  );
}
