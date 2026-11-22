import {
  Combobox,
  ComboboxChip,
  ComboboxChips,
  ComboboxChipsInput,
  ComboboxContent,
  ComboboxItem,
  ComboboxValue,
} from "nama-ui/components/combobox";
import { useState } from "react";
import { flushSync } from "react-dom";

import type { RowControl } from "@/shared/components/setting-row";
import type { PickItem } from "@/shared/lib/pick-item";

/**
 * The field of a `PickChips`: a chip per picked item, starting with the `held` values, and a list
 * of `items` to pick more from that typing narrows. Each picked one is a `name` form value, and
 * picking or removing one saves the form it's in, as ticking a checkbox would.
 */
export function ChipsField({
  control: { id, ...described },
  invalid,
  field: { name, items, held },
}: {
  control: RowControl;
  invalid: boolean;
  field: { name: string; items: PickItem[]; held: string[] };
}) {
  // Values, not items: a test may offer the same values under new labels.
  const [picked, setPicked] = useState(held);
  // In the list's order, so the chips and what's saved don't depend on the order they were picked.
  const pickedItems = items.filter((item) => picked.includes(item.value));
  // State, not a ref, so the list finds the modal `<dialog>` it must open inside, if any, once the
  // field is there: the page under one can't be seen or clicked.
  const [field, setField] = useState<HTMLDivElement | null>(null);
  const shown = (values: PickItem[]) => [
    ...values.map((item) => (
      <ComboboxChip key={item.value} remove={`Remove ${item.label}`}>
        {item.label}
      </ComboboxChip>
    )),
    <ComboboxChipsInput
      key="input"
      id={id}
      aria-invalid={invalid}
      placeholder={values.length === 0 ? "None" : undefined}
      {...described}
    />,
  ];
  const chips = (
    <ComboboxChips ref={setField}>
      <ComboboxValue>{shown}</ComboboxValue>
    </ComboboxChips>
  );
  const list = (
    <ComboboxContent
      anchor={field}
      container={field?.closest("dialog") ?? undefined}
      empty="Nothing matches."
    >
      {(item: PickItem) => (
        <ComboboxItem key={item.value} value={item}>
          {item.label}
        </ComboboxItem>
      )}
    </ComboboxContent>
  );
  return (
    <Combobox
      multiple
      autoHighlight
      items={items}
      value={pickedItems}
      name={name}
      itemToStringValue={(item) => item.value}
      isItemEqualToValue={(item, value) => item.value === value.value}
      onValueChange={(next) => {
        // The hidden inputs the form sends render from `picked`, so it's set before saving.
        flushSync(() => {
          setPicked(next.map((item) => item.value));
        });
        field?.closest("form")?.requestSubmit();
      }}
    >
      {chips}
      {list}
    </Combobox>
  );
}
