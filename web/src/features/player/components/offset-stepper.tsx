import { Minus, Plus } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { OFFSET_STEP_MS, offsetText } from "@/features/player/lib/subtitle-offset";

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
