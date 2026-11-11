import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { Ban, Power, PowerOff } from "lucide-react";

import { StateLabel } from "@/components/state-label";
import { Button } from "@/components/ui/button";
import { PluginService, type PluginSummary, type Setting } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { refusal, refusalOf } from "@/lib/forms";
import { pluginName, pluginsKey } from "@/lib/plugins";

/**
 * Whether the Plugin `summary` is enabled, a button to change that, and why the Server refused to,
 * for a wrapping flex row. A Plugin the Server refused only says so. A refusal names a setting by
 * its label in `settings`, or by its key without them, as on the list.
 */
export function PluginToggle({
  summary,
  settings,
}: {
  summary: PluginSummary;
  settings?: Setting[];
}) {
  if (summary.refusedReason !== "") return <StateLabel label="Refused" Icon={Ban} />;
  return <EnableOrDisable summary={summary} settings={settings} />;
}

function EnableOrDisable({
  summary,
  settings,
}: {
  summary: PluginSummary;
  settings: Setting[] | undefined;
}) {
  const queryClient = useQueryClient();
  // Refused or not, the list and the details page may be out of date.
  const onSettled = () => queryClient.invalidateQueries({ queryKey: pluginsKey });
  const enable = useMutation(PluginService.method.enablePlugin, { onSettled });
  const disable = useMutation(PluginService.method.disablePlugin, { onSettled });
  const { change, action, ...state } = summary.enabled
    ? { change: disable, action: "Disable", label: "Enabled", Icon: Power }
    : { change: enable, action: "Enable", label: "Disabled", Icon: PowerOff };
  // A fragment: the alert takes a whole line of the header, which wraps.
  return (
    <>
      <div className="flex items-center gap-2">
        <StateLabel {...state} />
        <Button
          variant="ghost"
          aria-label={`${action} ${pluginName(summary)}`}
          disabled={change.isPending}
          onClick={() => {
            change.mutate({ id: summary.id });
          }}
        >
          {action}
        </Button>
      </div>
      {change.isError && (
        <p role="alert" className="w-full text-sm text-pretty text-destructive">
          {refusedChange(change.error, `${action.toLowerCase()} ${pluginName(summary)}`, settings)}
        </p>
      )}
    </>
  );
}

/** Why `doing`, enabling or disabling a Plugin, failed, by its Refusal (web/AGENTS.md). */
function refusedChange(error: unknown, doing: string, settings: Setting[] | undefined) {
  const { field, params } = refusalOf(error);
  // The Server names a setting as `settings.<key>`.
  const key = field.replace(/^settings\./u, "");
  const label = settings?.find((setting) => setting.key === key)?.label ?? key;
  const { fulfillerPluginId, titleKind } = params;
  return refusal(
    error,
    doing,
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
