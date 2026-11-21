import { SettingRow } from "@/shared/components/setting-row";
import { Input } from "@/shared/ui/input";

/** A password field on its row, with what's wrong with what was typed under its hint. */
export function PasswordRow({
  label,
  hint,
  name,
  autoComplete,
  error,
}: {
  label: string;
  hint?: string;
  name: string;
  autoComplete: "current-password" | "new-password";
  error: string | undefined;
}) {
  return (
    <SettingRow label={label} hint={hint} error={error}>
      {(control) => (
        <span className="flex w-40 md:w-60">
          <Input
            {...control}
            name={name}
            type="password"
            autoComplete={autoComplete}
            aria-invalid={error !== undefined}
          />
        </span>
      )}
    </SettingRow>
  );
}
