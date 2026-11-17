import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { FormEnd } from "@/components/form-field";
import { SettingInput, settingsTyped } from "@/components/setting-fields";
import { refusedSettings } from "@/components/setting-refusals";
import { TestButton, testPassed, useSettingsTest } from "@/components/settings-test";
import { PluginService, type PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { onSubmitted } from "@/lib/forms";
import { type PluginDetails, pluginsKey } from "@/lib/plugins";

/**
 * The form an Admin changes `plugin`'s settings with, checked against its schema as they type. A
 * Plugin with a settings check can test what was typed, which offers the choices for its choice
 * settings.
 */
export function PluginSettings({ plugin }: { plugin: PluginDetails }) {
  const queryClient = useQueryClient();
  const update = useMutation(PluginService.method.updatePluginSettings, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: pluginsKey }),
  });
  const { choices, test } = useSettingsTest();
  // One message at a time: each resets the other.
  const errors = settingErrors(update.error, test.error);
  return (
    <form
      className="mt-4 flex max-w-md flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        test.reset();
        update.mutate({ id: plugin.summary.id, settings: settingsTyped(typed, plugin.settings) });
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

/**
 * Why the Server refused to save or test the settings, by whichever it refused. A `null` error
 * refused nothing.
 */
function settingErrors(saving: unknown, testing: unknown) {
  if (saving !== null) return refusedSettings(saving, "save the settings");
  return testing === null ? {} : refusedSettings(testing, "test the settings");
}

/** What saving did, by whether the Plugin `summary` runs with the new settings now. */
function saved(summary: PluginSummary | undefined) {
  return summary?.enabled === true
    ? "Saved, and in use now."
    : "Saved. They’re used once the Plugin is enabled.";
}
