import { type Setting, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { BoolInput } from "@/shared/components/setting-input/bool-input";
import { SetSecret } from "@/shared/components/setting-input/set-secret";
import { TypedInput } from "@/shared/components/setting-input/typed-input";
import { blankIsMissing } from "@/shared/lib/setting-values";

/** The input of a setting whose value is typed or ticked on its own, with no list to pick from. */
export function OwnInput({
  setting,
  error,
  checksRequired,
  forgettable,
}: {
  setting: Setting;
  error: string | undefined;
  checksRequired: boolean;
  forgettable: boolean;
}) {
  if (setting.kind === SettingKind.BOOL) {
    return <BoolInput setting={setting} />;
  }
  if (setting.secretSet) {
    return <SetSecret {...{ setting, error, forgettable }} />;
  }
  const required = checksRequired && blankIsMissing(setting);
  return <TypedInput setting={setting} error={error} required={required} />;
}
