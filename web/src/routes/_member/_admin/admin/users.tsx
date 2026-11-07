import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { CreateUser } from "@/components/create-user";
import { Page } from "@/components/page";
import { UserRow } from "@/components/user-row";
import { UserService } from "@/gen/nama/v1/user_pb";

export const Route = createFileRoute("/_member/_admin/admin/users")({
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(UserService.method.listUsers, {}, { transport }),
      staleTime: "static",
    }),
  component: Users,
});

function Users() {
  const { data } = useSuspenseQuery(UserService.method.listUsers, {});
  const { signedIn } = Route.useRouteContext();
  return (
    <Page heading="Users">
      {/* Pulled out by the rows' own padding, so the names line up with the heading. */}
      <ul className="mt-8 -mx-3 flex max-w-2xl flex-col">
        {data.users.map((user) => (
          <UserRow key={user.id} user={user} you={signedIn.user} />
        ))}
      </ul>
      <section aria-labelledby="create-user" className="mt-12 max-w-sm">
        <h2 id="create-user" className="font-medium">
          Create a User
        </h2>
        <CreateUser />
      </section>
    </Page>
  );
}
