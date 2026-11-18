import { ForgetButton } from "@/features/settings/components/forget-button";
import { usePersonalChanges } from "@/features/settings/hooks/use-personal-changes";
import type { PersonalSettings as Personal } from "@/gen/nama/v1/user_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { SettingInput } from "@/shared/components/setting-input/setting-input";
import { onSubmitted } from "@/shared/lib/forms";
import { settingsTyped } from "@/shared/lib/setting-values";

/** The form a Plugin's Personal settings are typed in, to save or forget. */
export function PersonalForm({ plugin }: { plugin: Personal }) {
  const { update, forget, errors } = usePersonalChanges(plugin.name);
  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        forget.reset();
        const settings = settingsTyped(typed, plugin.settings);
        update.mutate({ pluginId: plugin.pluginId, settings });
      })}
    >
      {plugin.settings.map((setting) => (
        // No choices: the Kernel refuses a CHOICE Personal setting (ADR 0046).
        <SettingInput
          key={setting.key}
          setting={setting}
          error={errors[setting.key]}
          checksRequired
        />
      ))}
      <FormEnd
        action={update.isPending ? "Saving…" : "Save"}
        variant="ghost"
        pending={update.isPending}
        done={update.isSuccess ? "Saved." : undefined}
        refusal={errors["form"]}
      >
        <ForgetButton
          name={plugin.name}
          disabled={forget.isPending}
          onClick={() => {
            update.reset();
            forget.mutate({ pluginId: plugin.pluginId });
          }}
        />
      </FormEnd>
    </form>
  );
}
