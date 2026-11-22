import { Select, SelectContent, SelectItem, SelectTrigger } from "nama-ui/components/select";
import { useId } from "react";

import type { Library } from "@/gen/nama/v1/library_pb";
import { useModalContainer } from "@/shared/hooks/use-modal-container";

/**
 * Picks the Library whose Items show, or every one's (`undefined`), calling `onPick` with it. Where
 * there's room it stays at the top of the pane while the list scrolls under it.
 */
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
  // Its list goes inside Settings' dialog, as only what's inside one shows over it.
  const [field, container] = useModalContainer();
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
      <SelectTrigger ref={field} id={id} />
      <SelectContent container={container}>{items}</SelectContent>
    </Select>
  );
  return (
    <div data-sticks className="top-0 z-10 bg-background px-4 pt-4 pb-3 md:sticky md:px-6 md:pt-2">
      {/* The field fills what the label leaves, up to a dropdown's width. */}
      <div className="flex max-w-80 items-center gap-3">
        <label htmlFor={id} className="text-sm font-medium">
          Library
        </label>
        {select}
      </div>
    </div>
  );
}
