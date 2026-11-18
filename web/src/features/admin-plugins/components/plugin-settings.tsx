import { TestButton } from "@/features/admin-plugins/components/test-button";
import { useSettingsTest } from "@/features/admin-plugins/data/use-settings-test";
import { useUpdatePluginSettings } from "@/features/admin-plugins/data/use-update-plugin-settings";
import { saved, settingErrors } from "@/features/admin-plugins/lib/setting-errors";
import { testPassed } from "@/features/admin-plugins/lib/test-passed";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { SettingInput } from "@/shared/components/setting-input/setting-input";
import { onSubmitted } from "@/shared/lib/forms";
import type { PluginDetails } from "@/shared/lib/plugins";
import { forgottenTyped, settingsTyped } from "@/shared/lib/setting-values";

/**
 * The form an Admin changes `plugin`'s settings with, checked against its schema as they type. A
 * Plugin with a settings check can test what was typed, which offers the choices for its choice
 * settings.
 */
export function PluginSettings({ plugin }: { plugin: PluginDetails }) {
  const update = useUpdatePluginSettings();
  const { choices, test } = useSettingsTest();
  // One message at a time: each resets the other.
  const errors = settingErrors(update.error, test.error);
  return (
    <form
      className="mt-4 flex max-w-md flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        test.reset();
        update.mutate({
          id: plugin.summary.id,
          settings: settingsTyped(typed, plugin.settings),
          forgottenSecrets: forgottenTyped(typed),
        });
      })}
    >
      {plugin.settings.map((setting) => (
        <SettingInput
          key={setting.key}
          setting={setting}
          // A passing test that offered nothing for a setting still unlocks it, empty.
          choices={choices && (choices[setting.key]?.choices ?? [])}
          error={errors[setting.key]}
          checksRequired={plugin.summary.enabled}
          forgettable
        />
      ))}
      <FormEnd
        action={update.isPending ? "Saving…" : "Save"}
        variant="ghost"
        pending={update.isPending}
        done={update.isSuccess ? saved(update.data.plugin?.summary) : testPassed(test.isSuccess)}
        refusal={errors["form"]}
      >
        <TestButton
          plugin={plugin}
          pending={test.isPending}
          onTest={(typed) => {
            update.reset();
            test.mutate({ id: plugin.summary.id, settings: settingsTyped(typed, plugin.settings) });
          }}
        />
      </FormEnd>
    </form>
  );
}
