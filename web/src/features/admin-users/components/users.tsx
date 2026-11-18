import { CreateUser } from "@/features/admin-users/components/create-user";
import { DefaultPermissionsEditor } from "@/features/admin-users/components/default-permissions-editor";
import { UserRow } from "@/features/admin-users/components/user-row";
import { useUsers } from "@/features/admin-users/data/use-users";
import { Invites } from "@/features/invites/components/invites";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { AdminList } from "@/shared/components/admin-list/admin-list";
import { Page } from "@/shared/components/page";

/** The Users page: the Users, a form to create one, the Invites and the default Permissions. */
export function Users({ you }: { you: User | undefined }) {
  const users = useUsers();
  return (
    <Page heading="Users">
      <AdminList>
        {users.map((user) => (
          <UserRow key={user.id} user={user} you={you} />
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
        <Invites you={you} />
      </section>
      <section aria-labelledby="default-permissions" className="mt-12 max-w-sm">
        <h2 id="default-permissions" className="font-medium">
          Default Permissions
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          What new Users get. Changing them leaves the Users already here as they are.
        </p>
        <DefaultPermissionsEditor you={you} />
      </section>
    </Page>
  );
}
