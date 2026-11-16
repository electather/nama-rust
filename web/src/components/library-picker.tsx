import { useQuery } from "@connectrpc/connect-query";
import { useId } from "react";

import { type Library, LibraryService } from "@/gen/nama/v1/library_pb";
import type { Setting } from "@/gen/nama/v1/plugin_pb";

/**
 * A Libraries setting's input: a checkbox per Library on the Server, by name, ticked for the ones
 * it holds. Each ticked one is a `setting.key` form value, its Library's id. A held id that no
 * Library has any more isn't shown, so saving drops it.
 */
export function LibraryPicker({
  setting,
  hint,
  error,
}: {
  setting: Setting;
  hint: string;
  error: string | undefined;
}) {
  const id = useId();
  const { data, isError } = useQuery(LibraryService.method.listLibraries, {});
  const held = setting.value?.value.case === "libraryIds" ? setting.value.value.value.ids : [];
  const described = `${id}-hint ${id}-error`;
  return (
    <fieldset className="flex flex-col gap-2" aria-describedby={described}>
      <legend className="mb-2 text-sm font-medium">{setting.label}</legend>
      {data === undefined ? (
        <Unlisted name={setting.key} held={held} failed={isError} />
      ) : (
        data.libraries.map((library) => (
          <LibraryBox
            key={String(library.id)}
            library={library}
            name={setting.key}
            checked={held.includes(library.id)}
            invalid={error !== undefined}
            described={described}
          />
        ))
      )}
      <p id={`${id}-hint`} className="text-sm text-pretty text-muted-foreground empty:hidden">
        {data?.libraries.length === 0 ? "There are no Libraries yet." : hint}
      </p>
      {/* Always there, so screen readers announce an error as it appears. */}
      <p id={`${id}-error`} role="alert" className="text-sm text-destructive empty:hidden">
        {error}
      </p>
    </fieldset>
  );
}

/** One Library's checkbox, sending its id as a `name` form value when ticked. */
function LibraryBox({
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

/**
 * While the Libraries load, or if they `failed` to, says so and sends the `held` ids as they were:
 * the Server keeps a left-out value only for a Secret, so sending none would clear them.
 */
function Unlisted({ name, held, failed }: { name: string; held: bigint[]; failed: boolean }) {
  return (
    <>
      <p className="text-sm text-muted-foreground">
        {failed
          ? "The Libraries couldn’t be listed, so saving keeps the ones picked."
          : "Loading the Libraries…"}
      </p>
      {held.map((id) => (
        <input key={String(id)} type="hidden" name={name} value={String(id)} />
      ))}
    </>
  );
}
