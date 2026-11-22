import { Switch } from "nama-ui/components/switch";
import type { ReactNode } from "react";

import { SettingRow } from "@/shared/components/setting-row";

/** A setting that's on or off: its row, with a switch at the end. */
export function SwitchRow({
  setting,
  label,
  hint,
  error,
  checked,
  disabled = false,
  onChange,
}: {
  setting?: string;
  label: string;
  hint?: ReactNode;
  error?: string | undefined;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <SettingRow setting={setting} label={label} hint={hint} error={error}>
      {(control) => (
        <Switch {...control} checked={checked} disabled={disabled} onCheckedChange={onChange} />
      )}
    </SettingRow>
  );
}
