import { ChipsField } from "@/shared/components/setting-input/chips-field";
import { SettingRow } from "@/shared/components/setting-row";
import type { PickItem } from "@/shared/lib/pick-item";

/**
 * A setting picked from `items`, several at once: its row, with a field of chips for the picked
 * ones, starting with the `held` values. Each is a `name` form value.
 */
export function PickChips({
  label,
  hint,
  error,
  name,
  items,
  held,
}: {
  label: string;
  hint: string;
  error: string | undefined;
  name: string;
  items: PickItem[];
  held: string[];
}) {
  return (
    <SettingRow wide label={label} hint={hint} error={error} size="field">
      {(control) => (
        <span className="flex w-full">
          <ChipsField
            control={control}
            invalid={error !== undefined}
            field={{ name, items, held }}
          />
        </span>
      )}
    </SettingRow>
  );
}
