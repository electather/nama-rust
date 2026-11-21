import { useId } from "react";

import { Switch } from "@/shared/ui/switch";

/** Whether a Job runs on its schedule, sent with its form as `enabled`. */
export function EnabledSwitch({ on, onChange }: { on: boolean; onChange: (on: boolean) => void }) {
  const id = useId();
  return (
    <div className="flex items-center justify-between gap-2.5">
      <span id={id} className="text-sm font-medium">
        Enabled
      </span>
      <Switch name="enabled" aria-labelledby={id} checked={on} onCheckedChange={onChange} />
    </div>
  );
}
