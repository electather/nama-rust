import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { FormEnd } from "@/components/form-field";
import { PluginLogo } from "@/components/plugin-at-a-glance";
import { SettingInput, refusedSettings, settingsTyped } from "@/components/setting-fields";
import { Button } from "@/components/ui/button";
import { type PersonalSettings as Personal, UserService } from "@/gen/nama/v1/user_pb";
import { onSubmitted } from "@/lib/forms";

const personalKey = createConnectQueryKey({
  schema: UserService.method.listPersonalSettings,
  cardinality: "finite",
});

/**
 * The signed-in User's own Personal settings, a card per enabled Plugin that declares any, such as
 * their ntfy topic. Nothing while no Plugin does.
 */
export function PersonalSettings() {
  const { data, dataUpdatedAt } = useQuery(UserService.method.listPersonalSettings);
  if (!data || data.plugins.length === 0) return null;
  return (
    <section aria-labelledby="personal" className="mt-12 max-w-md">
      <h2 id="personal" className="font-medium">
        Personal settings
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        Only you see these, and a Plugin uses them only for you.
      </p>
      <ul className="mt-4 flex flex-col gap-4">
        {data.plugins.map((plugin) => (
          <PluginCard key={plugin.pluginId} plugin={plugin} listedAt={dataUpdatedAt} />
        ))}
      </ul>
    </section>
  );
}

/** One Plugin's Personal settings to save or forget, as listed at `listedAt`. */
function PluginCard({ plugin, listedAt }: { plugin: Personal; listedAt: number }) {
  const { update, forget, errors } = usePersonalChanges(plugin.name);
  const headingId = `personal-${plugin.pluginId}`;
  return (
    <li aria-labelledby={headingId} className="rounded-xl border bg-surface-raised p-4.5">
      <CardHeading plugin={plugin} id={headingId} />
      <form
        // A new form once the list changes, so a forgotten value leaves its input too.
        key={listedAt}
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
          <Button
            type="button"
            variant="ghost"
            aria-label={`Forget your ${plugin.name} settings`}
            disabled={forget.isPending}
            onClick={() => {
              update.reset();
              forget.mutate({ pluginId: plugin.pluginId });
            }}
          >
            Forget
          </Button>
        </FormEnd>
      </form>
    </li>
  );
}

/** A card's title: the Plugin's logo and name. */
function CardHeading({ plugin, id }: { plugin: Personal; id: string }) {
  return (
    <div className="flex items-center gap-3">
      <PluginLogo plugin={{ id: plugin.pluginId, name: plugin.name, logoUrl: plugin.logoUrl }} />
      <h3 id={id} className="font-medium">
        {plugin.name}
      </h3>
    </div>
  );
}

/**
 * Saving and forgetting the Plugin `name`'s Personal settings, each listing them again once it's
 * done, and why the Server refused the last one, by setting.
 */
function usePersonalChanges(name: string) {
  const queryClient = useQueryClient();
  const onSuccess = () => queryClient.invalidateQueries({ queryKey: personalKey });
  const update = useMutation(UserService.method.updatePersonalSettings, { onSuccess });
  const forget = useMutation(UserService.method.deletePersonalSettings, { onSuccess });
  return { update, forget, errors: settingErrors(update.error, forget.error, name) };
}

/** Why the Server refused to save or forget `name`'s settings, by whichever it refused. */
function settingErrors(saving: unknown, forgetting: unknown, name: string) {
  if (saving !== null) return refusedSettings(saving, `save your ${name} settings`);
  return forgetting === null ? {} : refusedSettings(forgetting, `forget your ${name} settings`);
}
