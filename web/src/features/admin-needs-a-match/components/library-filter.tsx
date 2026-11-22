import { Select, SelectContent, SelectItem, SelectTrigger } from "nama-ui/components/select";
import { useId } from "react";

import type { Library } from "@/gen/nama/v1/library_pb";

/** Picks the Library whose Items show, or every one's (`undefined`), calling `onPick` with it. */
export function LibraryFilter({
  libraries,
  picked,
  onPick,
}: {
  libraries: Library[];
  picked: string | undefined;
  onPick: (library: string | undefined) => void;
}) {
  const id = useId();
  const options = [
    { value: "", label: "All Libraries" },
    ...libraries.map((library) => ({ value: String(library.id), label: library.name })),
  ];
  const items = options.map((option) => (
    <SelectItem key={option.value} value={option.value}>
      {option.label}
    </SelectItem>
  ));
  const select = (
    <Select
      items={options}
      value={picked ?? ""}
      onValueChange={(library) => {
        onPick(library === null || library === "" ? undefined : library);
      }}
    >
      <SelectTrigger id={id} />
      <SelectContent>{items}</SelectContent>
    </Select>
  );
  return (
    // The field fills what the label leaves, up to a dropdown's width.
    <div className="mt-8 flex max-w-80 items-center gap-3">
      <label htmlFor={id} className="text-sm font-medium">
        Library
      </label>
      {select}
    </div>
  );
}
