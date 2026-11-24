import { useState } from "react";

import { CardHeading } from "@/features/settings/components/card-heading";
import { ForgetButton } from "@/features/settings/components/forget-button";
import { usePersonalChanges } from "@/features/settings/hooks/use-personal-changes";
import { personalSetting } from "@/features/settings/lib/server-defined";
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
        // Each row is drawn by the input its kind takes, so `?setting=` finds this box around it,
        // which draws the hairline between rows in its place. The row is last in its box, so the
        // box puts its bottom padding back, keeping a found row's highlight even, and gives it
        // back in the row's place when it's the last box.
        <div
          key={setting.key}
          data-setting={personalSetting(plugin.pluginId, setting.key)}
          className="pb-3.5 not-first:border-t last:-mb-3.5"
        >
          {/* No choices: the Kernel refuses a CHOICE Personal setting (ADR 0046). */}
          <SettingInput setting={setting} error={errors[setting.key]} checksRequired />
        </div>
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
      {/* The last row gives back its bottom padding, so the message puts it back. */}
      <p role="alert" className="pt-3.5 text-sm text-destructive empty:hidden">
        {errors["form"]}
      </p>
    </form>
  );
}
