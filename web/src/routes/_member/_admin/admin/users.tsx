import { createFileRoute } from "@tanstack/react-router";

import { Users } from "@/features/admin-users/components/users";
import { invitesQuery } from "@/features/invites/data/use-invites";
import { defaultPermissionsQuery } from "@/shared/hooks/use-default-permissions";
import { usersQuery } from "@/shared/hooks/use-users";

export const Route = createFileRoute("/_member/_admin/admin/users")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query(usersQuery(transport)),
      queryClient.query(defaultPermissionsQuery(transport)),
      queryClient.query(invitesQuery(transport)),
    ]),
  component: UsersRoute,
});

function UsersRoute() {
  const { signedIn } = Route.useRouteContext();
  return <Users you={signedIn.user} />;
}
