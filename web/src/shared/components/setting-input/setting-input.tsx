import { type Setting, type SettingChoices, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { OwnInput } from "@/shared/components/setting-input/own-input";
import { PickedInput } from "@/shared/components/setting-input/picked-input";

const pickedKinds = new Set<SettingKind>([
  SettingKind.CHOICE,
  SettingKind.CHOICES,
  SettingKind.LIBRARIES,
]);

/**
 * One setting's input, by its kind. Shared by the Admin's Plugin settings and a User's Personal
 * settings, which a Plugin's Manifest declares alike.
 */
export function SettingInput({
  setting,
  offered,
  error,
  checksRequired,
  forgettable = false,
}: {
  setting: Setting;
  /** The choices the last passing settings test offered, by setting key. Never for Personal settings. */
  offered?: Record<string, SettingChoices> | undefined;
  error: string | undefined;
  /** Whether a required setting must be filled in. Not for a disabled Plugin, checked when enabled. */
  checksRequired: boolean;
  /** Whether a set secret can be forgotten on its own. Personal settings forget all of theirs at once. */
  forgettable?: boolean;
}) {
  if (pickedKinds.has(setting.kind)) {
    return <PickedInput {...{ setting, offered, error, checksRequired }} />;
  }
  return <OwnInput {...{ setting, error, checksRequired, forgettable }} />;
}
