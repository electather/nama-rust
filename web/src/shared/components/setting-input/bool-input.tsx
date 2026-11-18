import { useId } from "react";

import type { Setting } from "@/gen/nama/v1/plugin_pb";

/** A checkbox for a bool setting, with its description under it. */
export function BoolInput({ setting }: { setting: Setting }) {
  const hintId = useId();
  const shown = setting.value ?? setting.defaultValue;
  return (
    <div className="flex flex-col gap-2">
      <label className="flex items-center gap-3 text-sm font-medium">
        <input
          type="checkbox"
          name={setting.key}
          defaultChecked={shown?.value.case === "boolValue" && shown.value.value}
          aria-describedby={hintId}
          className="size-4 accent-foreground"
        />
        {setting.label}
      </label>
      <p id={hintId} className="text-sm text-pretty text-muted-foreground empty:hidden">
        {setting.description}
      </p>
    </div>
  );
}
