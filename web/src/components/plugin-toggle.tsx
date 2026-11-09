import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { Ban, Power, PowerOff } from "lucide-react";

import { StateLabel } from "@/components/state-label";
import { Button } from "@/components/ui/button";
import { type Plugin, PluginService, type PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { refusal, refusalOf } from "@/lib/forms";
import { pluginsKey } from "@/lib/plugins";

/**
 * Whether `plugin` is enabled, a button to change that, and why the Server refused to, for a
 * wrapping flex row. A Plugin the Server refused only says so.
 */
export function PluginToggle({ plugin }: { plugin: PluginSummary | Plugin }) {
  if (plugin.refusedReason !== "") return <StateLabel label="Refused" Icon={Ban} />;
  return <EnableOrDisable plugin={plugin} />;
}

function EnableOrDisable({ plugin }: { plugin: PluginSummary | Plugin }) {
  const queryClient = useQueryClient();
  // Refused or not, the list and the details page may be out of date.
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
function refusedChange(error: unknown, plugin: PluginSummary | Plugin, verb: string) {
  const { field, params } = refusalOf(error);
  // The Server names a setting as `settings.<key>`.
  const key = field.replace(/^settings\./u, "");
  // The list doesn't have the settings, so names the setting by its key.
  const settings = "settings" in plugin ? plugin.settings : [];
  const label = settings.find((setting) => setting.key === key)?.label ?? key;
  const { fulfillerPluginId, titleKind } = params;
  return refusal(
    error,
    `${verb} ${plugin.id}`,
    {},
    {
      [RefusalReason.LAST_AUTHENTICATOR]: {
        form: "It can’t be disabled while no other enabled Plugin lets Users sign in.",
      },
      [RefusalReason.FIELD_REQUIRED]: { form: `It needs ${label} set before it can be enabled.` },
      [RefusalReason.FULFILLER_TAKEN]: {
        form: `It can’t be enabled while ${fulfillerPluginId} is the ${titleKind} Fulfiller. Disable ${fulfillerPluginId} first.`,
      },
      [RefusalReason.SETTING_WRONG_KIND]: {
        form: `Its ${label} setting isn’t valid, so it can’t be enabled.`,
      },
    },
  ).form;
}
