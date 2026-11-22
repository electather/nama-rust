import { LinkedUserItem } from "@hub/features/links/components/linked-user-item";
import { useLinkedUsers } from "@hub/features/links/data/users";

/** The signed-in Account's Linked Users, or what stands in for them while they load or if they can't. */
export function LinkedUsers() {
  const users = useLinkedUsers();
  if (users.isPending) {
    return <p className="text-sm text-muted-foreground">Looking up your Users…</p>;
  }
  if (users.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        The Hub couldn’t list your Users just now. Reload the page to try again.
      </p>
    );
  }
  return users.data.users.length === 0 ? (
    <p className="text-sm text-muted-foreground">No User is Linked yet.</p>
  ) : (
    <ul className="flex flex-col divide-y">
      {users.data.users.map((user) => (
        <LinkedUserItem key={user.id} user={user} />
      ))}
    </ul>
  );
}
