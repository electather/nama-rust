import { Minus, Plus } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { Slider } from "nama-ui/components/slider";

import { clampSpeed, FASTEST, SLOWEST, SPEED_STEP, speedValue } from "@/features/player/lib/speeds";

/** A slider from `SLOWEST` to `FASTEST`, with − and + a step either side. */
export function SpeedSlider({ rate, choose }: { rate: number; choose: (rate: number) => void }) {
  return (
    <div className="flex items-center gap-3">
      <Button
        variant="secondary"
        size="round"
        aria-label="Slower"
        disabled={rate <= SLOWEST}
        onClick={() => {
          choose(clampSpeed(rate - SPEED_STEP));
        }}
      >
        <Minus aria-hidden />
      </Button>
      <Slider
        label="Speed"
        valueText={speedValue(rate)}
        value={rate}
        min={SLOWEST}
        max={FASTEST}
        step={SPEED_STEP}
        onValueChange={(value) => {
          choose(clampSpeed(value));
        }}
      />
      <Button
        variant="secondary"
        size="round"
        aria-label="Faster"
        disabled={rate >= FASTEST}
        onClick={() => {
          choose(clampSpeed(rate + SPEED_STEP));
        }}
      >
        <Plus aria-hidden />
      </Button>
    </div>
  );
}
