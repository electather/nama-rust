import { SkipForward } from "lucide-react";
import { Switch } from "nama-ui/components/switch";

import { useAutoplayOn } from "@/features/player/data/use-autoplay-on";
import { useSetAutoplay } from "@/shared/hooks/use-set-autoplay";

/** Whether the next Episode plays by itself, the same choice as in Settings. */
export function AutoplayRow() {
  const on = useAutoplayOn();
  const { mutate } = useSetAutoplay();
  return (
    // oxlint-disable-next-line jsx-a11y/label-has-associated-control -- Base UI's Switch is the control, which the rule can't see.
    <label className="flex h-10 w-full shrink-0 cursor-pointer items-center gap-2.5 rounded-md px-2.5 transition-colors select-none hover:bg-muted">
      <SkipForward aria-hidden className="size-4 text-muted-foreground" />
      <span className="flex-1">Autoplay next Episode</span>
      <Switch
        checked={on}
        onCheckedChange={(checked) => {
          mutate({ on: checked });
        }}
      />
    </label>
  );
}
