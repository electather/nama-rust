import type { Setting } from "@/gen/nama/v1/plugin_pb";
import { PickChips } from "@/shared/components/setting-input/pick-chips";
import { UnlistedLibraries } from "@/shared/components/setting-input/unlisted-libraries";
import { SettingRow } from "@/shared/components/setting-row";
import { useLibrariesToPick } from "@/shared/hooks/use-libraries-to-pick";

/**
 * A Libraries setting's input: a chip per Library it holds, picked from the Server's by name. Each
 * picked one is a `setting.key` form value, its Library's id. A held id that no Library has any more
 * isn't shown, so saving drops it.
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
  if (data === undefined) {
    return (
      <SettingRow wide label={setting.label} hint={hint} error={error}>
        {() => <UnlistedLibraries name={setting.key} held={held} failed={isError} />}
      </SettingRow>
    );
  }
  return (
    <PickChips
      label={setting.label}
      hint={data.libraries.length === 0 ? "There are no Libraries yet." : hint}
      error={error}
      name={setting.key}
      items={data.libraries.map(({ id, name }) => ({ value: String(id), label: name }))}
      held={held.map(String)}
    />
  );
}
