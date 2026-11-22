import { Select, SelectContent, SelectItem, SelectTrigger } from "nama-ui/components/select";
import { useState } from "react";
import { flushSync } from "react-dom";

import type { FieldControl } from "@/shared/components/form-field/field";
import { changed } from "@/shared/lib/forms";
import type { SelectOption } from "@/shared/lib/select-option";

/** What a `FormSelect` sends and starts on, and who hears it change. */
export interface SelectChoice {
  name: string;
  options: SelectOption[];
  defaultValue?: string;
  value?: string;
  onValueChange?: (value: string) => void;
  required?: boolean;
  disabled?: boolean;
}

/**
 * The dropdown of a `FormSelect`, wired to its label by `control`: `ui/select.tsx`, sending the
 * chosen option as `name`. In a form that saves as it's changed, picking saves.
 */
export function FieldSelect({
  control,
  choice: { name, options, defaultValue, value, onValueChange, required = true, disabled },
}: {
  control: FieldControl;
  choice: SelectChoice;
}) {
  const [chosen, setChosen] = useState(defaultValue);
  // State, not a ref, so the list finds the modal `<dialog>` it must open inside, if any.
  const [trigger, setTrigger] = useState<HTMLButtonElement | null>(null);
  const items = options.map((option) => (
    <SelectItem key={option.value} value={option.value} disabled={option.disabled}>
      {option.label}
    </SelectItem>
  ));
  return (
    <Select
      items={options}
      name={name}
      value={value ?? chosen}
      required={required}
      disabled={disabled}
      onValueChange={(picked) => {
        // The hidden input the form sends renders from `chosen`, so it's set before saving.
        flushSync(() => {
          setChosen(picked ?? "");
        });
        onValueChange?.(picked ?? "");
        changed(trigger);
      }}
    >
      <SelectTrigger
        ref={setTrigger}
        // Named by `aria-labelledby`, not by its id in the label's `for`: a dropdown's trigger is a
        // button, which tools that find a field by its label don't take a `for` to.
        aria-labelledby={`${control.id}-label`}
        aria-invalid={control["aria-invalid"]}
        aria-describedby={control["aria-describedby"]}
      />
      <SelectContent container={trigger?.closest("dialog") ?? undefined}>{items}</SelectContent>
    </Select>
  );
}
