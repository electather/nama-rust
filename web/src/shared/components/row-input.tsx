import { Input } from "nama-ui/components/input";
import { NumberInput } from "nama-ui/components/number-field";
import type { ComponentProps, FocusEventHandler, KeyboardEventHandler } from "react";

import type { RowControl } from "@/shared/components/setting-row";

/** What a `TypedSettingRow` hands its field, beyond the field's own attributes. */
export interface RowInputState {
  control: RowControl;
  value: string;
  invalid: boolean;
  onType: (value: string) => void;
  onBlur: FocusEventHandler<HTMLInputElement>;
  onKeyDown: KeyboardEventHandler<HTMLInputElement>;
  /** − or + on a number field, which leave the field where it is. */
  onStep: (value: string) => void;
}

/**
 * The field of a `TypedSettingRow`: a text input, or for `type="number"`, `ui/number-field.tsx`
 * with − and + at its end.
 */
export function RowInput({
  input: { type, min, ...input },
  state: { control, value, invalid, onType, onBlur, onKeyDown, onStep },
}: {
  input: Omit<ComponentProps<typeof Input>, "value" | "onChange" | "onBlur" | "onKeyDown">;
  state: RowInputState;
}) {
  const events = { onBlur, onKeyDown, "aria-invalid": invalid };
  if (type === "number") {
    return (
      <NumberInput
        {...control}
        {...events}
        placeholder={input.placeholder}
        min={min === undefined ? undefined : Number(min)}
        value={value}
        onValueChange={onType}
        onStep={onStep}
      />
    );
  }
  return (
    <Input
      {...input}
      {...control}
      {...events}
      type={type}
      min={min}
      value={value}
      onChange={(event) => {
        onType(event.target.value);
      }}
    />
  );
}
