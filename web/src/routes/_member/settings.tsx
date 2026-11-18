import { createFileRoute } from "@tanstack/react-router";

import { SettingsPage } from "@/features/settings/components/settings-page";
import { imageSettingsQuery } from "@/features/settings/data/use-image-settings";
import { serverSettingsQuery } from "@/features/settings/data/use-server-settings";
import { trackPreferencesQuery } from "@/features/settings/data/use-track-preferences";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { yourDevicesQuery } from "@/shared/hooks/use-devices";
import { may } from "@/shared/lib/permissions";

export const Route = createFileRoute("/_member/settings")({
  loader: ({ context: { queryClient, transport, signedIn } }) =>
    Promise.all([
      queryClient.query(yourDevicesQuery(transport)),
      queryClient.query(trackPreferencesQuery(transport)),
      may(signedIn.user, Permission.MANAGE_SETTINGS) &&
        queryClient.query(serverSettingsQuery(transport)),
      may(signedIn.user, Permission.MANAGE_SETTINGS) &&
        queryClient.query(imageSettingsQuery(transport)),
    ]),
  component: Settings,
});

function Settings() {
  const { signedIn } = Route.useRouteContext();
  return <SettingsPage user={signedIn.user} />;
}
