import type { Setting } from "@/gen/nama/v1/plugin_pb";
import { PickBox } from "@/shared/components/setting-input/pick-box";
import { PickList } from "@/shared/components/setting-input/pick-list";
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
  const { data, isError } = useLibrariesToPick();
  const held = setting.value?.value.case === "libraryIds" ? setting.value.value.value.ids : [];
  return (
    <PickList
      label={setting.label}
      hint={data?.libraries.length === 0 ? "There are no Libraries yet." : hint}
      error={error}
    >
      {(box) =>
        data === undefined ? (
          <UnlistedLibraries name={setting.key} held={held} failed={isError} />
        ) : (
          data.libraries.map((library) => (
            <PickBox
              key={String(library.id)}
              name={setting.key}
              value={String(library.id)}
              checked={held.includes(library.id)}
              {...box}
            >
              {library.name}
            </PickBox>
          ))
        )
      }
    </PickList>
  );
}
