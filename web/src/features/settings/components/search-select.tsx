import {
  Combobox,
  ComboboxContent,
  ComboboxItem,
  ComboboxTrigger,
} from "nama-ui/components/combobox";

import type { RowControl } from "@/shared/components/setting-row";
import { useModalContainer } from "@/shared/hooks/use-modal-container";

/**
 * A dropdown at the end of a setting's row for more `options` than anyone would scroll through:
 * its list opens under a search, named `search`, that narrows it.
 */
export function SearchSelect({
  control,
  options,
  value,
  search,
  onChange,
}: {
  control: RowControl;
  options: { value: string; label: string }[];
  value: string;
  search: string;
  onChange: (value: string) => void;
}) {
  const [box, container] = useModalContainer();
  const choice = (option: (typeof options)[number]) => (
    <ComboboxItem key={option.value} value={option}>
      {option.label}
    </ComboboxItem>
  );
  return (
    <span ref={box} className="flex w-full">
      <Combobox
        items={options}
        value={options.find((option) => option.value === value) ?? null}
        onValueChange={(picked) => {
          // Picking the chosen one again clears it, which a setting can't be.
          onChange(picked?.value ?? value);
        }}
      >
        <ComboboxTrigger {...control} />
        <ComboboxContent container={container} search={search} empty="Nothing matches.">
          {choice}
        </ComboboxContent>
      </Combobox>
    </span>
  );
}
