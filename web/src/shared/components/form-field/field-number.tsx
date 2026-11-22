import type { Input } from "nama-ui/components/input";
import { NumberInput } from "nama-ui/components/number-field";
import type { ComponentProps } from "react";

import type { FieldControl } from "@/shared/components/form-field/field";

/** A bound an `<input>` gives as text or a number, as a number. */
function bound(value: string | number | undefined) {
  return value === undefined ? undefined : Number(value);
}

/**
 * The control of a `FormField` of `type="number"`: `ui/number-field.tsx`, given the attributes the
 * text input would have had, wired to its label by `control`. `onStep` hears − and +.
 */
export function FieldNumber({
  control,
  input: { min, max, step, defaultValue, type: _type, variant: _variant, ...input },
  onStep,
}: {
  control: FieldControl;
  input: Omit<ComponentProps<typeof Input>, "value" | "onChange">;
  onStep: ((value: string, field: HTMLInputElement | null) => void) | undefined;
}) {
  return (
    <NumberInput
      {...control}
      {...input}
      min={bound(min)}
      max={bound(max)}
      step={bound(step)}
      defaultValue={typeof defaultValue === "object" ? undefined : defaultValue}
      onStep={onStep}
    />
  );
}
