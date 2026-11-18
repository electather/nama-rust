import type { Library } from "@/gen/nama/v1/library_pb";

/** One Library's checkbox, sending its id as a `name` form value when ticked. */
export function LibraryBox({
  library,
  name,
  checked,
  invalid,
  described,
}: {
  library: Library;
  name: string;
  checked: boolean;
  invalid: boolean;
  described: string;
}) {
  return (
    <label className="flex items-center gap-3 text-sm">
      <input
        type="checkbox"
        name={name}
        value={String(library.id)}
        defaultChecked={checked}
        aria-invalid={invalid}
        aria-describedby={described}
        className="size-4 accent-foreground"
      />
      {library.name}
    </label>
  );
}
