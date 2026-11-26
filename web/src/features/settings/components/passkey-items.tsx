import { passkeyError } from "@/features/auth/lib/hub-errors";
import { HubRefusal } from "@/features/settings/components/hub-refusal";
import { PasskeyItem } from "@/features/settings/components/passkey-item";
import { usePasskeys } from "@/features/settings/data/use-passkeys";
import { useRemovePasskey } from "@/features/settings/data/use-remove-passkey";
import { AlertText } from "@/shared/components/form-field/alert-text";

/** The Account's passkeys, each with Remove, or what a passkey is while it has none. */
export function PasskeyItems() {
  const passkeys = usePasskeys();
  const remove = useRemovePasskey();
  return (
    <>
      <ul aria-label="Passkeys" className="-mb-1.5 flex flex-col empty:hidden">
        {passkeys.map((passkey) => (
          <PasskeyItem
            key={passkey.id}
            passkey={passkey}
            removing={remove.isPending}
            onRemove={() => {
              remove.mutate({ passkeyId: passkey.id });
            }}
          />
        ))}
      </ul>
      {passkeys.length === 0 && (
        <p className="py-2.5 text-sm text-muted-foreground">
          A passkey signs you in with your fingerprint, face or screen lock instead of a password.
        </p>
      )}
      <AlertText
        error={
          <HubRefusal
            error={remove.error}
            said={passkeyError(remove.error)}
            toDo="to change your passkeys"
          />
        }
      />
    </>
  );
}
