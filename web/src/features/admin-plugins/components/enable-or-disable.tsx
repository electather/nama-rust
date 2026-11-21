import { Power, PowerOff } from "lucide-react";

import { GrantedPermissions } from "@/features/admin-plugins/components/granted-permissions";
import { useDisablePlugin } from "@/features/admin-plugins/data/use-disable-plugin";
import { useEnablePlugin } from "@/features/admin-plugins/data/use-enable-plugin";
import { showsHeld } from "@/features/admin-plugins/lib/plugin-permissions";
import { refusedChange } from "@/features/admin-plugins/lib/refused-change";
import { PluginKind, type PluginSummary, type Setting } from "@/gen/nama/v1/plugin_pb";
import { StateLabel } from "@/shared/components/state-label";
import { pluginName } from "@/shared/lib/plugins";
import { Button } from "@/shared/ui/button";

/** A Plugin's state and a button to enable or disable it, with why the Server refused. */
export function EnableOrDisable({
  summary,
  settings,
}: {
  summary: PluginSummary;
  settings: Setting[] | undefined;
}) {
  const enable = useEnablePlugin();
  const disable = useDisablePlugin();
  const { change, action, ...state } = summary.enabled
    ? { change: disable, action: "Disable", label: "Enabled", Icon: Power }
    : { change: enable, action: "Enable", label: "Disabled", Icon: PowerOff };
  // A fragment: the granted Permissions and the alert each take a whole line of the header, which wraps.
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
      {showsHeld(summary) && (
        <GrantedPermissions
          granted={summary.grantedPermissions}
          builtIn={summary.kind === PluginKind.BUILT_IN}
        />
      )}
      {change.isError && (
        <p role="alert" className="w-full text-sm text-pretty text-destructive">
          {refusedChange(change.error, `${action.toLowerCase()} ${pluginName(summary)}`, settings)}
        </p>
      )}
    </>
  );
}
