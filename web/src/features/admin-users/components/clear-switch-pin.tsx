import { useClearUserSwitchPin } from "@/features/admin-users/data/use-clear-user-switch-pin";
import { clearSwitchPinRefusal } from "@/features/admin-users/lib/user-refusals";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";

/** Clears `user`'s Switch PIN, such as one they forgot, once confirmed. */
export function ClearSwitchPin({ user }: { user: User }) {
  const clear = useClearUserSwitchPin();
  return (
    <ConfirmDialog
      action="Clear Switch PIN"
      name={`Clear Switch PIN for ${user.name}`}
      title={`Clear ${user.name}’s Switch PIN?`}
      description={`Switching to ${user.name} then needs no Switch PIN, until they set a new one.`}
      onConfirm={async () => {
        await clear.mutateAsync({ userId: user.id });
      }}
      refusal={(error) => clearSwitchPinRefusal(error, user.name)}
    />
  );
}
