import { LinkedUsers } from "@hub/features/links/components/linked-users";

/** The signed-in Account's Linked Users, each on its Server, and Unlinking them. */
export function UserList() {
  return (
    <section aria-labelledby="your-users" className="flex flex-col gap-2">
      <h2 id="your-users" className="text-lg/6 font-medium">
        Your Users
      </h2>
      <LinkedUsers />
    </section>
  );
}
