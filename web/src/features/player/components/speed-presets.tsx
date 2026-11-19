import { SpeedPreset } from "@/features/player/components/speed-preset";
import { PRESETS } from "@/features/player/lib/speeds";

/** The presets in a row, the one at `rate` lit. */
export function SpeedPresets({ rate, choose }: { rate: number; choose: (rate: number) => void }) {
  return (
    <fieldset aria-label="Preset speeds" className="grid min-w-0 grid-cols-5 gap-1.5">
      {PRESETS.map((preset) => (
        <SpeedPreset key={preset} preset={preset} on={preset === rate} choose={choose} />
      ))}
    </fieldset>
  );
}
