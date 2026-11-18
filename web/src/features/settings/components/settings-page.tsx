import { DefaultQualities } from "@/features/settings/components/default-qualities";
import { DevicesSection } from "@/features/settings/components/devices-section";
import { ImagesSection } from "@/features/settings/components/images-section";
import { NotificationPreferences } from "@/features/settings/components/notification-preferences";
import { PasswordSection } from "@/features/settings/components/password-section";
import { PersonalSettings } from "@/features/settings/components/personal-settings";
import { PluginConnections } from "@/features/settings/components/plugin-connections";
import { ServerSection } from "@/features/settings/components/server-section";
import { TrackPreferences } from "@/features/settings/components/track-preferences";
import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import { Page } from "@/shared/components/page";
import { may } from "@/shared/lib/permissions";

/** The signed-in `user`'s own settings, and the Server's for one who may manage them. */
export function SettingsPage({ user }: { user: User | undefined }) {
  return (
    <Page heading="Settings">
      <PasswordSection />
      <DevicesSection />
      <TrackPreferences />
      {may(user, Permission.CHOOSE_QUALITY) && <DefaultQualities />}
      <PluginConnections />
      <PersonalSettings />
      <NotificationPreferences />
      {may(user, Permission.MANAGE_SETTINGS) && <ServerSection />}
      {may(user, Permission.MANAGE_SETTINGS) && <ImagesSection />}
    </Page>
  );
}
