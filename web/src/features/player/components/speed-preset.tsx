import { presetName } from "@/features/player/lib/speeds";
import { Button } from "@/shared/ui/button";

/** One preset, pressed while it's the speed, with "Normal" under 1.0. */
export function SpeedPreset({
  preset,
  on,
  choose,
}: {
  preset: number;
  on: boolean;
  choose: (rate: number) => void;
}) {
  return (
    <span className="flex flex-col items-stretch gap-1">
      <Button
        variant="secondary"
        aria-pressed={on}
        onClick={() => {
          choose(preset);
        }}
      >
        {presetName(preset)}
      </Button>
      {preset === 1 && <span className="px-1 text-xs text-muted-foreground">Normal</span>}
    </span>
  );
}
