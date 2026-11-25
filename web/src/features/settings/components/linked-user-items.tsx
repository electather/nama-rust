import { useLinkedUsers } from "@/features/auth/data/use-linked-users";
import { LinkedUserItem } from "@/features/settings/components/linked-user-item";

/** The signed-in Account's Linked Users, each on its Server, or that it has none yet. */
export function LinkedUserItems() {
  const users = useLinkedUsers();
  if (users.length === 0) {
    return <p className="py-2.5 text-sm text-muted-foreground">No User is Linked yet.</p>;
  }
  return (
    <ul aria-label="Your Users" className="-mb-1.5 flex flex-col">
      {users.map((user) => (
        <LinkedUserItem key={user.id} user={user} />
      ))}
    </ul>
  );
}
