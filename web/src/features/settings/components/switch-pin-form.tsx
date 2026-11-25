import { Input } from "nama-ui/components/input";

import { SwitchPinEnd } from "@/features/settings/components/switch-pin-end";
import { useSwitchPinChange } from "@/features/settings/hooks/use-switch-pin-change";
import { signinLabels } from "@/features/settings/lib/pane-labels";
import { SettingRow } from "@/shared/components/setting-row";
import { onSubmitted } from "@/shared/lib/forms";
import { rowText } from "@/shared/lib/setting-text";

/**
 * The form the signed-in User sets, changes or clears their own Switch PIN with. The Server never
 * says whether one is set, so both are always offered.
 */
export function SwitchPinForm() {
  const { errors, done, pending, set, clear } = useSwitchPinChange();
  return (
    <form
      noValidate
      className="flex flex-col"
      onSubmit={onSubmitted((typed, form) => {
        void set(typed, form);
      })}
    >
      <SettingRow
        wide
        {...rowText(signinLabels, "switchPin")}
        error={errors.switchPin}
        size="field"
      >
        {(control) => (
          <Input
            {...control}
            name="switchPin"
            type="password"
            inputMode="numeric"
            autoComplete="off"
            maxLength={6}
            aria-invalid={errors.switchPin !== undefined}
          />
        )}
      </SettingRow>
      <SwitchPinEnd
        pending={pending}
        done={done}
        onClear={(form) => {
          void clear(form);
        }}
      />
      <p role="alert" className="pt-2.5 text-end text-xs text-destructive empty:hidden">
        {errors.form}
      </p>
    </form>
  );
}
