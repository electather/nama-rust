import { ConnectionItem } from "@/features/settings/components/connection-item";
import { NoConnections } from "@/features/settings/components/no-connections";
import { NotificationRows } from "@/features/settings/components/notification-rows";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PersonalSettings } from "@/features/settings/components/personal-settings";
import { usePluginConnections } from "@/features/settings/data/use-plugin-connections";
import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/**
 * The Plugins that can message the signed-in `user` about their Requests, such as Telegram, each
 * to connect or disconnect, what they message about, and their own settings for a Plugin.
 */
export function ConnectionsPane({ user }: { user: User | undefined }) {
  const connections = usePluginConnections();
  return (
    <PaneFrame section="connections">
      {connections.length === 0 ? (
        <NoConnections managesPlugins={may(user, Permission.MANAGE_SETTINGS)} />
      ) : (
        <ul aria-label="Plugins" className="-my-3.5 flex flex-col">
          {connections.map((connection) => (
            <ConnectionItem key={connection.pluginId} connection={connection} />
          ))}
        </ul>
      )}
      <NotificationRows />
      <PersonalSettings />
    </PaneFrame>
  );
}
