import { DeleteAccountNote } from "@hub/features/account/components/delete-account-note";
import type { useDeleteAccount } from "@hub/features/account/data/sessions";
import { Button } from "nama-ui/components/button";

/** The words of Delete Account's dialog, why the Hub refused, Cancel, and deleting. */
export function DeleteAccountForm({
  id,
  remove,
  close,
}: {
  id: string;
  remove: ReturnType<typeof useDeleteAccount>;
  close: () => void;
}) {
  return (
    <form
      className="flex flex-col gap-4 p-6"
      onSubmit={(event) => {
        event.preventDefault();
        remove.mutate({});
      }}
    >
      <h2 id={`${id}-title`} className="text-lg leading-none font-semibold text-balance">
        Delete your Account?
      </h2>
      <p id={`${id}-description`} className="text-sm text-pretty text-muted-foreground">
        The Hub deletes your Watch State, Watchlist and viewing history, and Unlinks your Users, and
        the Servers you Linked with everyone on them. A User who signs in only with your Account
        can’t sign in until their Server’s Admin gives them a password. Counts of what everyone
        finished, which don’t name you, stay. This can’t be undone.
      </p>
      <DeleteAccountNote error={remove.error} />
      <div className="flex flex-col-reverse gap-2 *:w-full sm:flex-row sm:justify-end sm:*:w-auto">
        <Button variant="outline" disabled={remove.isPending} onClick={close}>
          Cancel
        </Button>
        <Button type="submit" variant="destructive" disabled={remove.isPending}>
          {remove.isPending ? "Deleting…" : "Delete Account"}
        </Button>
      </div>
    </form>
  );
}
