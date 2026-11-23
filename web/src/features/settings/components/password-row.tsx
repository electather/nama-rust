import { Input } from "nama-ui/components/input";

import { SettingRow } from "@/shared/components/setting-row";

/** A password field on its row, with what's wrong with what was typed under its hint. */
export function PasswordRow({
  setting,
  label,
  hint,
  name,
  autoComplete,
  error,
}: {
  setting: string;
  label: string;
  hint: string | undefined;
  name: string;
  autoComplete: "current-password" | "new-password";
  error: string | undefined;
}) {
  return (
    <SettingRow wide setting={setting} label={label} hint={hint} error={error} size="field">
      {(control) => (
        <Input
          {...control}
          name={name}
          type="password"
          autoComplete={autoComplete}
          aria-invalid={error !== undefined}
        />
      )}
    </SettingRow>
  );
}
