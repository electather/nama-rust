import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";
import { useId } from "react";

import { Page } from "@/components/page";
import { PluginToggle } from "@/components/plugin-toggle";
import {
  type Plugin,
  PluginKind,
  PluginService,
  type Setting,
  type SettingValue,
} from "@/gen/nama/v1/plugin_pb";

export const Route = createFileRoute("/_member/_admin/admin/plugins")({
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(PluginService.method.listPlugins, {}, { transport }),
      staleTime: "static",
    }),
  component: Plugins,
});

function Plugins() {
  const { plugins } = useSuspenseQuery(PluginService.method.listPlugins, {}).data;
  return (
    <Page heading="Plugins">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        Plugins add to what the Server can do, such as finding Movies and Shows on TMDB.
      </p>
      <div className="mt-8 flex max-w-3xl flex-col divide-y">
        {plugins.map((plugin) => (
          <PluginSection key={plugin.id} plugin={plugin} />
        ))}
      </div>
    </Page>
  );
}

function PluginSection({ plugin }: { plugin: Plugin }) {
  const heading = useId();
  return (
    <section aria-labelledby={heading} className="py-6">
      <div className="flex flex-wrap items-start justify-between gap-x-4 gap-y-2">
        <div>
          <h2 id={heading} className="font-medium">
            {plugin.id}
          </h2>
          <p className="text-sm text-muted-foreground">
            {plugin.kind === PluginKind.BUILT_IN ? "Built-in, version " : "Version "}
            {plugin.version}
          </p>
        </div>
        <PluginToggle plugin={plugin} />
      </div>
      {plugin.settings.length > 0 ? (
        <dl className="mt-4 flex flex-col gap-4">
          {plugin.settings.map((setting) => (
            <div
              key={setting.key}
              className="flex flex-col gap-1 sm:flex-row sm:justify-between sm:gap-6"
            >
              <dt>
                <span className="text-sm font-medium">{setting.label}</span>
                {setting.description && (
                  <span className="block text-sm text-pretty text-muted-foreground">
                    {setting.description}
                  </span>
                )}
              </dt>
              <dd className="shrink-0 text-sm wrap-anywhere sm:text-end">{shownValue(setting)}</dd>
            </div>
          ))}
        </dl>
      ) : (
        <p className="mt-4 text-sm text-muted-foreground">No settings.</p>
      )}
    </section>
  );
}

/**
 * A setting's value in words: a secret only as set or not (the Server sends no value for one), a
 * missing value as its default.
 */
function shownValue({ required, secretSet, value, defaultValue }: Setting) {
  if (secretSet) return "Set";
  if (value) return valueText(value);
  return defaultValue ? `${valueText(defaultValue)} (default)` : notSet(required);
}

/** A required setting that isn't set is why its Plugin can't be enabled, so it says so. */
const notSet = (required: boolean) => (required ? "Not set (required)" : "Not set");

function valueText({ value }: SettingValue) {
  if (value.case === "boolValue") return value.value ? "On" : "Off";
  return value.case === undefined ? "Not set" : String(value.value);
}
