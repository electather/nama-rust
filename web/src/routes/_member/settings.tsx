import { createQueryOptions } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { ChangePassword } from "@/components/change-password";
import { DeviceList } from "@/components/device-list";
import { Page } from "@/components/page";
import { ServerUrl } from "@/components/server-url";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { may } from "@/lib/permissions";

export const Route = createFileRoute("/_member/settings")({
  loader: ({ context: { queryClient, transport, signedIn } }) =>
    Promise.all([
      queryClient.query({
        ...createQueryOptions(UserService.method.listDevices, {}, { transport }),
        staleTime: "static",
      }),
      may(signedIn.user, Permission.MANAGE_SETTINGS) &&
        queryClient.query({
          ...createQueryOptions(ServerService.method.getServerSettings, {}, { transport }),
          staleTime: "static",
        }),
    ]),
  component: Settings,
});

function Settings() {
  const { signedIn } = Route.useRouteContext();
  return (
    <Page heading="Settings">
      <section aria-labelledby="password" className="mt-10 max-w-sm">
        <h2 id="password" className="font-medium">
          Password
        </h2>
        <ChangePassword />
      </section>
      <section aria-labelledby="devices" className="mt-12 max-w-2xl">
        <h2 id="devices" className="font-medium">
          Devices
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Where you’re signed in. Revoking a Device signs it out.
        </p>
        <div className="mt-4">
          <DeviceList />
        </div>
      </section>
      {may(signedIn.user, Permission.MANAGE_SETTINGS) && (
        <section aria-labelledby="server" className="mt-12 max-w-md">
          <h2 id="server" className="font-medium">
            Server
          </h2>
          <ServerUrl />
        </section>
      )}
    </Page>
  );
}
