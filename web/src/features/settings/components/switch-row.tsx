import type { ReactNode } from "react";

import { SettingRow } from "@/shared/components/setting-row";
import { Switch } from "@/shared/ui/switch";

/** A setting that's on or off: its row, with a switch at the end. */
export function SwitchRow({
  label,
  hint,
  error,
  checked,
  disabled = false,
  onChange,
}: {
  label: string;
  hint?: ReactNode;
  error?: string | undefined;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <SettingRow label={label} hint={hint} error={error}>
      {(control) => (
        <Switch {...control} checked={checked} disabled={disabled} onCheckedChange={onChange} />
      )}
    </SettingRow>
  );
}
