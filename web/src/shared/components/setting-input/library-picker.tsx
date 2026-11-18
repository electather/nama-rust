import { useId } from "react";

import type { Setting } from "@/gen/nama/v1/plugin_pb";
import { LibraryBox } from "@/shared/components/setting-input/library-box";
import { UnlistedLibraries } from "@/shared/components/setting-input/unlisted-libraries";
import { useLibrariesToPick } from "@/shared/hooks/use-libraries-to-pick";

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
  const { data, isError } = useLibrariesToPick();
  const held = setting.value?.value.case === "libraryIds" ? setting.value.value.value.ids : [];
  const described = `${id}-hint ${id}-error`;
  return (
    <fieldset className="flex flex-col gap-2" aria-describedby={described}>
      <legend className="mb-2 text-sm font-medium">{setting.label}</legend>
      {data === undefined ? (
        <UnlistedLibraries name={setting.key} held={held} failed={isError} />
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
