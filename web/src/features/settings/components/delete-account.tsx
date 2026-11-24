import { ConnectError } from "@connectrpc/connect";

import { HubRefusal } from "@/features/settings/components/hub-refusal";
import { useDeleteAccount } from "@/features/settings/data/use-delete-account";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";

const GOES =
  "The Hub deletes your Watch State, Watchlist and viewing history, and Unlinks your Users, and the Servers you Linked with everyone on them.";

/** What deleting the Account takes with it, and deleting it once confirmed. */
export function DeleteAccount() {
  const remove = useDeleteAccount();
  return (
    <div className="flex items-center gap-5 py-2.5 max-md:flex-col max-md:items-stretch">
      <p className="flex-1 text-xs text-pretty text-muted-foreground">
        {GOES} This can’t be undone.
      </p>
      <ConfirmDialog
        name="Delete Account"
        action="Delete Account"
        title="Delete your Account?"
        description={`${GOES} A User who signs in only with your Account can’t sign in until their Server’s Admin gives them a password. Counts of what everyone finished, which don’t name you, stay. This can’t be undone.`}
        onConfirm={async () => {
          await remove.mutateAsync({});
        }}
        refusal={deleteRefusal}
      />
    </div>
  );
}

/** Why the Hub didn't delete the Account, with the way back from a sign-in too old. */
function deleteRefusal(error: unknown) {
  return (
    <HubRefusal
      error={ConnectError.from(error)}
      said="The Hub couldn’t delete your Account just now. Try again."
      toDo="to delete your Account"
    />
  );
}
