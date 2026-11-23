import { Select, SelectContent, SelectItem, SelectTrigger } from "nama-ui/components/select";
import { useId } from "react";

import { useModalContainer } from "@/shared/hooks/use-modal-container";

/** A dropdown of the schedule picker, after the words that say what it picks, such as "Every". */
export function PickSelect({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: { value: number; label: string }[];
  value: number;
  onChange: (value: number) => void;
}) {
  const id = useId();
  const [box, container] = useModalContainer();
  const items = options.map((option) => ({ value: String(option.value), label: option.label }));
  const choices = items.map((item) => (
    <SelectItem key={item.value} value={item.value}>
      {item.label}
    </SelectItem>
  ));
  return (
    <span ref={box} className="flex items-center gap-2 text-sm">
      <span id={id}>{label}</span>
      <Select
        items={items}
        value={String(value)}
        onValueChange={(picked) => {
          onChange(Number(picked));
        }}
      >
        <SelectTrigger aria-labelledby={id} />
        <SelectContent container={container}>{choices}</SelectContent>
      </Select>
    </span>
  );
}
