import { Select, SelectContent, SelectItem, SelectTrigger } from "nama-ui/components/select";

import type { RowControl } from "@/shared/components/setting-row";
import { useModalContainer } from "@/shared/hooks/use-modal-container";

/** A dropdown at the end of a setting's row: `options` to pick from, the one valued `value` chosen. */
export function SelectBox({
  control,
  options,
  value,
  onChange,
}: {
  control: Partial<RowControl>;
  options: { value: string; label: string }[];
  value: string | undefined;
  onChange: (value: string) => void;
}) {
  const [box, container] = useModalContainer();
  const choices = options.map((option) => (
    <SelectItem key={option.value} value={option.value}>
      {option.label}
    </SelectItem>
  ));
  return (
    <span ref={box} className="flex w-full">
      <Select
        items={options}
        value={value}
        onValueChange={(picked) => {
          onChange(picked ?? "");
        }}
      >
        <SelectTrigger {...control} />
        <SelectContent container={container}>{choices}</SelectContent>
      </Select>
    </span>
  );
}
