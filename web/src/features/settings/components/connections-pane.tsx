import { BrowserPush } from "@/features/settings/components/browser-push";
import { ConnectionList } from "@/features/settings/components/connection-list";
import { NotificationRows } from "@/features/settings/components/notification-rows";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PersonalSettings } from "@/features/settings/components/personal-settings";
import { SectionBoundary } from "@/features/settings/components/section-boundary";
import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/**
 * The Plugins that can message the signed-in `user` about their Requests, such as Telegram, each
 * to connect or disconnect, whether this browser shows notifications too, what they message about,
 * and their own settings for a Plugin. Each part shows as soon as it has loaded.
 */
export function ConnectionsPane({ user }: { user: User | undefined }) {
  return (
    <PaneFrame section="connections">
      <SectionBoundary what="your Connections">
        <ConnectionList managesPlugins={may(user, Permission.MANAGE_SETTINGS)} />
      </SectionBoundary>
      <SectionBoundary what="this browser’s notifications">
        <BrowserPush />
      </SectionBoundary>
      <SectionBoundary what="what you’re messaged about" rows={3}>
        <NotificationRows />
      </SectionBoundary>
      {/* Most Plugins have no settings of their own, so this loads without rows to replace. */}
      <SectionBoundary what="your Plugins’ own settings" rows={0}>
        <PersonalSettings />
      </SectionBoundary>
    </PaneFrame>
  );
}
