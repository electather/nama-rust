import { type Setting, type SettingChoices, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { ChoiceInput } from "@/shared/components/setting-input/choice-input";
import { ChoicesPicker } from "@/shared/components/setting-input/choices-picker";
import { LibraryPicker } from "@/shared/components/setting-input/library-picker";
import { hint } from "@/shared/lib/setting-values";

/** The input of a setting picked from a list: a Plugin's choices, or the Server's Libraries. */
export function PickedInput({
  setting,
  offered,
  error,
  checksRequired,
}: {
  setting: Setting;
  offered: Record<string, SettingChoices> | undefined;
  error: string | undefined;
  checksRequired: boolean;
}) {
  if (setting.kind === SettingKind.CHOICE) {
    const required = checksRequired && setting.required;
    const choices = offeredByTest(setting, offered);
    // A new element once the choices come, so it starts on the stored one.
    return (
      <ChoiceInput key={String(choices !== undefined)} {...{ setting, choices, error, required }} />
    );
  }
  if (setting.kind === SettingKind.CHOICES) {
    return (
      <ChoicesPicker setting={setting} offered={offeredOrListed(setting, offered)} error={error} />
    );
  }
  return <LibraryPicker setting={setting} hint={hint(setting)} error={error} />;
}

/** A CHOICE setting's choices: none before a passing test, which unlocks it even offering none. */
function offeredByTest({ key }: Setting, offered: Record<string, SettingChoices> | undefined) {
  return offered && (offered[key]?.choices ?? []);
}

/** A CHOICES setting's choices: a passing test's for it, which replace its Manifest's. */
function offeredOrListed(
  { key, choices }: Setting,
  offered: Record<string, SettingChoices> | undefined,
) {
  return offered?.[key]?.choices ?? choices;
}
