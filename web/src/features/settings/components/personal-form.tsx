import { useState } from "react";

import { CardHeading } from "@/features/settings/components/card-heading";
import { ForgetButton } from "@/features/settings/components/forget-button";
import { usePersonalChanges } from "@/features/settings/hooks/use-personal-changes";
import type { PersonalSettings as Personal } from "@/gen/nama/v1/user_pb";
import { SettingInput } from "@/shared/components/setting-input/setting-input";
import { useAutoSubmit } from "@/shared/hooks/use-auto-submit";
import { onSubmitted } from "@/shared/lib/forms";
import { settingsTyped } from "@/shared/lib/setting-values";

/**
 * The form a Plugin's Personal settings are typed in, under the Plugin's name as the heading
 * `headingId`. Each setting is saved as it's changed, and the button at the heading's end forgets
 * them all.
 */
export function PersonalForm({ plugin, headingId }: { plugin: Personal; headingId: string }) {
  const { update, forget, errors } = usePersonalChanges(plugin.name);
  const auto = useAutoSubmit();
  // How many times they were forgotten: the fields are made again each time, to start blank.
  const [forgotten, setForgotten] = useState(0);
  const head = (
    <div className="flex items-center justify-between gap-3">
      <CardHeading plugin={plugin} id={headingId} />
      <ForgetButton
        name={plugin.name}
        disabled={forget.isPending}
        onClick={() => {
          update.reset();
          forget.mutate(
            { pluginId: plugin.pluginId },
            {
              onSuccess: () => {
                setForgotten(forgotten + 1);
              },
            },
          );
        }}
      />
    </div>
  );
  const rows = (
    <div key={forgotten} className="flex flex-col">
      {plugin.settings.map((setting) => (
        // No choices: the Kernel refuses a CHOICE Personal setting (ADR 0046).
        <SettingInput
          key={setting.key}
          setting={setting}
          error={errors[setting.key]}
          checksRequired
        />
      ))}
    </div>
  );
  return (
    <form
      className="flex flex-col gap-1"
      {...auto}
      onSubmit={onSubmitted((typed) => {
        forget.reset();
        const settings = settingsTyped(typed, plugin.settings);
        update.mutate({ pluginId: plugin.pluginId, settings });
      })}
    >
      {head}
      {rows}
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {errors["form"]}
      </p>
    </form>
  );
}
