import { Switch } from "nama-ui/components/switch";

import type { Setting } from "@/gen/nama/v1/plugin_pb";
import { SettingRow } from "@/shared/components/setting-row";

/** A bool setting's row: its label and description, with a switch at the end. */
export function BoolInput({ setting }: { setting: Setting }) {
  const shown = setting.value ?? setting.defaultValue;
  return (
    <SettingRow label={setting.label} hint={setting.description}>
      {(control) => (
        <Switch
          {...control}
          name={setting.key}
          defaultChecked={shown?.value.case === "boolValue" && shown.value.value}
        />
      )}
    </SettingRow>
  );
}
