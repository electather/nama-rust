import { type Setting, type SettingChoices, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { ChoiceInput } from "@/shared/components/setting-input/choice-input";
import { ChoicesPicker } from "@/shared/components/setting-input/choices-picker";
import { LibraryPicker } from "@/shared/components/setting-input/library-picker";
import { hint } from "@/shared/lib/setting-values";

/** The input of a setting picked from a list: a Plugin's choices, or the Server's Libraries. */
export function PickedInput({
  setting,
  offered,
  checksSettings,
  error,
  checksRequired,
}: {
  setting: Setting;
  offered: Record<string, SettingChoices> | undefined;
  checksSettings: boolean;
  error: string | undefined;
  checksRequired: boolean;
}) {
  if (setting.kind === SettingKind.CHOICE) {
    const required = checksRequired && setting.required;
    const choices = choicesOf(setting, offered);
    // A new element once the choices come, so it starts on the stored one.
    return (
      <ChoiceInput key={String(choices !== undefined)} {...{ setting, choices, error, required }} />
    );
  }
  if (setting.kind === SettingKind.CHOICES) {
    return (
      <ChoicesPicker
        setting={setting}
        offered={offeredOrListed(setting, offered)}
        current={offeredNow(offered, checksSettings)}
        error={error}
      />
    );
  }
  return <LibraryPicker setting={setting} hint={hint(setting)} error={error} />;
}

/**
 * A CHOICE setting's choices: a passing test's for it, else its Manifest's. With neither, none
 * before a passing test, which unlocks it even offering none.
 */
function choicesOf({ key, choices }: Setting, offered: Record<string, SettingChoices> | undefined) {
  const listed = choices.length > 0 ? choices : undefined;
  return offered?.[key]?.choices ?? listed ?? (offered && []);
}

/** A CHOICES setting's choices: a passing test's for it, which replace its Manifest's. */
function offeredOrListed(
  { key, choices }: Setting,
  offered: Record<string, SettingChoices> | undefined,
) {
  return offered?.[key]?.choices ?? choices;
}

/** Whether a CHOICES setting's choices are what the Plugin offers now, not ones a test may replace. */
function offeredNow(offered: Record<string, SettingChoices> | undefined, checksSettings: boolean) {
  return offered !== undefined || !checksSettings;
}
