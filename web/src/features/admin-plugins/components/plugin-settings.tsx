import { TestButton } from "@/features/admin-plugins/components/test-button";
import { useOfferedProfiles } from "@/features/admin-plugins/data/use-offered-profiles";
import { useSettingsTest } from "@/features/admin-plugins/data/use-settings-test";
import { useUpdatePluginSettings } from "@/features/admin-plugins/data/use-update-plugin-settings";
import { settingErrors } from "@/features/admin-plugins/lib/setting-errors";
import { testPassed } from "@/features/admin-plugins/lib/test-passed";
import { SettingInput } from "@/shared/components/setting-input/setting-input";
import { useAutoSubmit } from "@/shared/hooks/use-auto-submit";
import { onSubmitted } from "@/shared/lib/forms";
import type { PluginDetails } from "@/shared/lib/plugins";
import { forgottenTyped, settingsTyped } from "@/shared/lib/setting-values";

/**
 * The form an Admin changes `plugin`'s settings with, under its heading. Each setting is saved as
 * it's changed, and a refusal shows under the setting it names. A Plugin with a settings check can
 * test what was typed from the button at the heading's end. Its choice settings offer what the
 * check offers on the stored settings, or on what was typed once it's tested.
 */
export function PluginSettings({ plugin }: { plugin: PluginDetails }) {
  const update = useUpdatePluginSettings();
  const { choices, test } = useSettingsTest();
  const stored = useOfferedProfiles(plugin);
  // What the stored settings offer, until a test of what's typed offers something else.
  const offered = choices ?? stored.data?.choices;
  const auto = useAutoSubmit();
  // One message at a time: each resets the other.
  const errors = settingErrors(update.error, test.error);
  const head = (
    <div className="flex items-center justify-between gap-3">
      <h2 id="settings" className="font-medium">
        Settings
      </h2>
      <TestButton
        plugin={plugin}
        pending={test.isPending}
        onTest={(typed) => {
          update.reset();
          test.mutate({ id: plugin.summary.id, settings: settingsTyped(typed, plugin.settings) });
        }}
      />
    </div>
  );
  const rows = (
    <div className="flex flex-col">
      {plugin.settings.map((setting) => (
        <SettingInput
          key={setting.key}
          setting={setting}
          offered={offered}
          checksSettings={plugin.checksSettings}
          error={errors[setting.key]}
          checksRequired={plugin.summary.enabled}
          forgettable
        />
      ))}
    </div>
  );
  return (
    <form
      className="flex flex-col gap-1"
      {...auto}
      onSubmit={onSubmitted((typed) => {
        test.reset();
        update.mutate({
          id: plugin.summary.id,
          settings: settingsTyped(typed, plugin.settings),
          forgottenSecrets: forgottenTyped(typed),
        });
      })}
    >
      {head}
      {rows}
      <output className="text-sm empty:hidden">{testPassed(test.isSuccess)}</output>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {errors["form"]}
      </p>
    </form>
  );
}
