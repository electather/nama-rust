import { Minus, Plus } from "lucide-react";

import { OFFSET_STEP_MS, offsetText } from "@/features/player/lib/subtitle-offset";
import { Button } from "@/shared/ui/button";

/** `offsetMs` between − and +, which `shift` it a step either way. */
export function OffsetStepper({
  offsetMs,
  shift,
}: {
  offsetMs: number;
  shift: ((by: number) => void) | undefined;
}) {
  return (
    <div className="flex items-center gap-1">
      <Button
        variant="secondary"
        size="round"
        aria-label="Show earlier"
        onClick={() => shift?.(-OFFSET_STEP_MS)}
      >
        <Minus aria-hidden />
      </Button>
      <output className="flex-1 text-center text-nowrap tabular-nums">
        {offsetText(offsetMs)}
      </output>
      <Button
        variant="secondary"
        size="round"
        aria-label="Show later"
        onClick={() => shift?.(OFFSET_STEP_MS)}
      >
        <Plus aria-hidden />
      </Button>
    </div>
  );
}
