import {
  Combobox,
  ComboboxContent,
  ComboboxItem,
  ComboboxTrigger,
} from "nama-ui/components/combobox";
import { useState } from "react";
import { flushSync } from "react-dom";

import type { FieldControl } from "@/shared/components/form-field/field";
import type { SelectChoice } from "@/shared/components/form-field/field-select";
import { changed } from "@/shared/lib/forms";
import type { SelectOption } from "@/shared/lib/select-option";

/**
 * `FieldSelect` for more options than anyone would scroll through: its list opens under a search
 * that narrows it by label or value. Sends the chosen option as `name`, and in a form that saves as
 * it's changed, picking saves.
 */
export function FieldSearchSelect({
  control,
  choice: { name, options, defaultValue, required },
}: {
  control: FieldControl;
  choice: Pick<SelectChoice, "name" | "options" | "defaultValue"> & { required: boolean };
}) {
  const [chosen, setChosen] = useState(
    () => options.find((option) => option.value === defaultValue) ?? null,
  );
  // State, not a ref, so the list finds the modal `<dialog>` it must open inside, if any.
  const [trigger, setTrigger] = useState<HTMLButtonElement | null>(null);
  return (
    <Combobox
      items={options}
      name={name}
      required={required}
      value={chosen}
      filter={matches}
      onValueChange={(picked) => {
        // Picking the chosen one again clears it, which this field can't be.
        if (picked === null) {
          return;
        }
        // The hidden input the form sends renders from `chosen`, so it's set before saving.
        flushSync(() => {
          setChosen(picked);
        });
        changed(trigger);
      }}
    >
      <ComboboxTrigger
        ref={setTrigger}
        aria-labelledby={`${control.id}-label`}
        aria-invalid={control["aria-invalid"]}
        aria-describedby={control["aria-describedby"]}
      />
      <ComboboxContent
        container={trigger?.closest("dialog") ?? undefined}
        search="Search"
        empty="Nothing matches."
      >
        {(option: SelectOption) => (
          <ComboboxItem key={option.value} value={option}>
            {option.label}
          </ComboboxItem>
        )}
      </ComboboxContent>
    </Combobox>
  );
}

/** Whether `option`'s label or value holds `query`, whatever its case. */
function matches(option: SelectOption, query: string) {
  const typed = query.toLocaleLowerCase();
  return [option.label, option.value].some((text) => text.toLocaleLowerCase().includes(typed));
}
