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
  return (
    <div className="mt-8 flex items-center gap-3">
      <label htmlFor={id} className="text-sm font-medium">
        Library
      </label>
      <select
        id={id}
        value={picked ?? ""}
        onChange={(event) => {
          const library = event.target.value;
          onPick(library === "" ? undefined : library);
        }}
        className="h-9 rounded-lg border border-input bg-transparent px-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/25"
      >
        <option value="">All Libraries</option>
        {libraries.map((library) => (
          <option key={library.id} value={String(library.id)}>
            {library.name}
          </option>
        ))}
      </select>
    </div>
  );
}
