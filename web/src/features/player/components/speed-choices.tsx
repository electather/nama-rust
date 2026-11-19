import { SpeedPresets } from "@/features/player/components/speed-presets";
import { SpeedSlider } from "@/features/player/components/speed-slider";
import { speedValue } from "@/features/player/lib/speeds";

/**
 * The speed at `rate`, to two places, a slider with − and + that set it finely, and the presets.
 * Choosing keeps the panel open, so a drag or a few taps can find the speed.
 */
export function SpeedChoices({ rate, choose }: { rate: number; choose: (rate: number) => void }) {
  return (
    <div className="flex flex-col gap-4 px-2.5 pt-2 pb-2.5">
      <output className="text-center text-xl font-semibold tabular-nums">{speedValue(rate)}</output>
      <SpeedSlider rate={rate} choose={choose} />
      <SpeedPresets rate={rate} choose={choose} />
    </div>
  );
}
