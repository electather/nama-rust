import type { ReactNode } from "react";

import { type Setting, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { FormField } from "@/shared/components/form-field/form-field";
import { changed } from "@/shared/lib/forms";
import { hint, valueText } from "@/shared/lib/setting-values";

/** The input attributes of each kind of setting that isn't plain text or a bool. */
const inputs: Partial<Record<SettingKind, { type: string; step?: number }>> = {
  [SettingKind.SECRET]: { type: "password" },
  [SettingKind.INTEGER]: { type: "number", step: 1 },
  [SettingKind.URL]: { type: "url" },
};

/** A text, secret, number or URL setting's input. A secret's value never comes back, so it starts blank. */
export function TypedInput({
  setting,
  error,
  required,
  focused,
  forgetting = false,
  hiddenInput,
}: {
  setting: Setting;
  error: string | undefined;
  required: boolean;
  /** Whether it takes the focus as it appears, in place of the button that was pressed. */
  focused?: boolean;
  /** Whether leaving a set secret blank forgets it, rather than keeping it. */
  forgetting?: boolean;
  /** A hidden input sent with it, kept inside its row. */
  hiddenInput?: ReactNode;
}) {
  return (
    <FormField
      row
      label={setting.label}
      name={setting.key}
      {...inputs[setting.kind]}
      min={setting.minValue?.toString()}
      max={setting.maxValue?.toString()}
      autoComplete="off"
      spellCheck={false}
      // oxlint-disable-next-line jsx-a11y/no-autofocus -- only set when pressing Replace or Forget swaps it in for the button, whose focus would otherwise be lost
      autoFocus={focused}
      defaultValue={valueText(setting.value)}
      required={required}
      hint={hint(setting, forgetting ? "forget" : "keep")}
      error={error}
      hiddenInput={hiddenInput}
      // A number's − and + don't leave the field, so each saves the form as a change would.
      onStep={(_value, field) => {
        changed(field);
      }}
    />
  );
}
