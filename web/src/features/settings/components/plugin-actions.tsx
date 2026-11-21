import { GrantedPermissions } from "@/features/admin-plugins/components/granted-permissions";
import { useDisablePlugin } from "@/features/admin-plugins/data/use-disable-plugin";
import { useEnablePlugin } from "@/features/admin-plugins/data/use-enable-plugin";
import { showsHeld } from "@/features/admin-plugins/lib/plugin-permissions";
import { refusedChange } from "@/features/admin-plugins/lib/refused-change";
import { EnableButton } from "@/features/settings/components/enable-button";
import { PluginMore } from "@/features/settings/components/plugin-more";
import { PluginKind } from "@/gen/nama/v1/plugin_pb";
import { type PluginDetails, pluginName } from "@/shared/lib/plugins";

/**
 * The end of a Plugin's header: the "…" with its links and Disable, and for a disabled one the
 * button that enables it, so the button says its state and no label has to. The Plugin Permissions
 * it holds and why the Server refused a change each take a line of the header under them.
 */
export function PluginActions({ plugin }: { plugin: PluginDetails }) {
  const { summary, settings } = plugin;
  const enable = useEnablePlugin();
  const disable = useDisablePlugin();
  const { id } = summary;
  const off = () => {
    disable.mutate({ id });
  };
  const { change, action, onDisable } = summary.enabled
    ? { change: disable, action: "disable", onDisable: off }
    : { change: enable, action: "enable", onDisable: undefined };
  return (
    <>
      <div className="ms-auto flex items-center gap-2">
        <PluginMore plugin={plugin} onDisable={onDisable} />
        <EnableButton
          summary={summary}
          settings={settings}
          pending={enable.isPending}
          onEnable={() => {
            enable.mutate({ id });
          }}
        />
      </div>
      {showsHeld(summary) && (
        <GrantedPermissions
          granted={summary.grantedPermissions}
          builtIn={summary.kind === PluginKind.BUILT_IN}
        />
      )}
      {change.isError && (
        <p role="alert" className="w-full text-sm text-pretty text-destructive">
          {refusedChange(change.error, `${action} ${pluginName(summary)}`, settings)}
        </p>
      )}
    </>
  );
}
