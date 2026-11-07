import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { Power, PowerOff } from "lucide-react";

import { StateLabel } from "@/components/state-label";
import { Button } from "@/components/ui/button";
import { type Plugin, PluginService } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { refusal, refusalOf } from "@/lib/forms";

const pluginsKey = createConnectQueryKey({
  schema: PluginService.method.listPlugins,
  cardinality: "finite",
});

/**
 * Whether `plugin` is enabled, a button to change that, and why the Server refused to, for a
 * wrapping flex row.
 */
export function PluginToggle({ plugin }: { plugin: Plugin }) {
  const queryClient = useQueryClient();
  // Refused or not, the list may be out of date.
  const onSettled = () => queryClient.invalidateQueries({ queryKey: pluginsKey });
  const enable = useMutation(PluginService.method.enablePlugin, { onSettled });
  const disable = useMutation(PluginService.method.disablePlugin, { onSettled });
  const { change, action, ...state } = plugin.enabled
    ? { change: disable, action: "Disable", label: "Enabled", Icon: Power }
    : { change: enable, action: "Enable", label: "Disabled", Icon: PowerOff };
  // A fragment: the alert takes a whole line of the header, which wraps.
  return (
    <>
      <div className="flex items-center gap-2">
        <StateLabel {...state} />
        <Button
          variant="ghost"
          aria-label={`${action} ${plugin.id}`}
          disabled={change.isPending}
          onClick={() => {
            change.mutate({ id: plugin.id });
          }}
        >
          {action}
        </Button>
      </div>
      {change.isError && (
        <p role="alert" className="w-full text-sm text-pretty text-destructive">
          {refusedChange(change.error, plugin, action.toLowerCase())}
        </p>
      )}
    </>
  );
}

/** Why `plugin` couldn't be enabled or disabled, by its Refusal (web/AGENTS.md). */
function refusedChange(error: unknown, plugin: Plugin, verb: string) {
  // The Server names a setting as `settings.<key>`.
  const key = refusalOf(error).field.replace(/^settings\./u, "");
  const label = plugin.settings.find((setting) => setting.key === key)?.label ?? key;
  return refusal(
    error,
    `${verb} ${plugin.id}`,
    {},
    {
      [RefusalReason.LAST_AUTHENTICATOR]: {
        form: "It can’t be disabled while no other enabled Plugin lets Users sign in.",
      },
      [RefusalReason.FIELD_REQUIRED]: { form: `It needs ${label} set before it can be enabled.` },
      [RefusalReason.SETTING_WRONG_KIND]: {
        form: `Its ${label} setting isn’t valid, so it can’t be enabled.`,
      },
    },
  ).form;
}
