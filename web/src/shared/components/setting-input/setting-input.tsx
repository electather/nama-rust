import { type Choice, type Setting, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { ChoiceInput } from "@/shared/components/setting-input/choice-input";
import { LibraryPicker } from "@/shared/components/setting-input/library-picker";
import { OwnInput } from "@/shared/components/setting-input/own-input";
import { hint } from "@/shared/lib/setting-values";

/**
 * One setting's input, by its kind. Shared by the Admin's Plugin settings and a User's Personal
 * settings, which a Plugin's Manifest declares alike.
 */
export function SettingInput({
  setting,
  choices,
  error,
  checksRequired,
  forgettable = false,
}: {
  setting: Setting;
  /** A CHOICE setting's options, once a settings test offers them. Never for Personal settings. */
  choices?: Choice[] | undefined;
  error: string | undefined;
  /** Whether a required setting must be filled in. Not for a disabled Plugin, checked when enabled. */
  checksRequired: boolean;
  /** Whether a set secret can be forgotten on its own. Personal settings forget all of theirs at once. */
  forgettable?: boolean;
}) {
  if (setting.kind === SettingKind.CHOICE) {
    const required = checksRequired && setting.required;
    // A new element once the choices come, so it starts on the stored one.
    return (
      <ChoiceInput key={String(choices !== undefined)} {...{ setting, choices, error, required }} />
    );
  }
  if (setting.kind === SettingKind.LIBRARIES) {
    return <LibraryPicker setting={setting} hint={hint(setting)} error={error} />;
  }
  return <OwnInput {...{ setting, error, checksRequired, forgettable }} />;
}
