import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { CreateUser } from "@/components/create-user";
import { DefaultPermissionsEditor } from "@/components/permissions-editor";
import { UserRow } from "@/components/user-row";
import { Invites } from "@/features/invites/components/invites";
import { UserService } from "@/gen/nama/v1/user_pb";
import { AdminList } from "@/shared/components/admin-list/admin-list";
import { Page } from "@/shared/components/page";

export const Route = createFileRoute("/_member/_admin/admin/users")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query({
        ...createQueryOptions(UserService.method.listUsers, {}, { transport }),
        staleTime: "static",
      }),
      queryClient.query({
        ...createQueryOptions(UserService.method.getDefaultPermissions, {}, { transport }),
        staleTime: "static",
      }),
      queryClient.query({
        ...createQueryOptions(UserService.method.listInvites, {}, { transport }),
        staleTime: "static",
      }),
    ]),
  component: Users,
});

function Users() {
  const { data } = useSuspenseQuery(UserService.method.listUsers, {});
  const { signedIn } = Route.useRouteContext();
  return (
    <Page heading="Users">
      <AdminList>
        {data.users.map((user) => (
          <UserRow key={user.id} user={user} you={signedIn.user} />
        ))}
      </AdminList>
      <section aria-labelledby="create-user" className="mt-12 max-w-sm">
        <h2 id="create-user" className="font-medium">
          Create a User
        </h2>
        <CreateUser />
      </section>
      <section aria-labelledby="invites" className="mt-12 max-w-sm">
        <h2 id="invites" className="font-medium">
          Invites
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          A link someone opens to create their own User, with the Permissions you pick.
        </p>
        <Invites you={signedIn.user} />
      </section>
      <section aria-labelledby="default-permissions" className="mt-12 max-w-sm">
        <h2 id="default-permissions" className="font-medium">
          Default Permissions
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          What new Users get. Changing them leaves the Users already here as they are.
        </p>
        <DefaultPermissionsEditor you={signedIn.user} />
      </section>
    </Page>
  );
}
