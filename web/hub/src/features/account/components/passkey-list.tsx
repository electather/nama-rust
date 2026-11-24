import { PasskeyNotice } from "@hub/features/account/components/passkey-notice";
import { PasskeyRow } from "@hub/features/account/components/passkey-row";
import { usePasskeys, useRemovePasskey } from "@hub/features/account/data/passkeys";

/** The Account's passkeys, each with a button that removes it, or what stands in for them. */
export function PasskeyList() {
  const listed = usePasskeys();
  const remove = useRemovePasskey();
  if (listed.isPending) {
    return <p className="text-sm text-muted-foreground">Looking up your passkeys…</p>;
  }
  if (listed.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        The Hub couldn’t list your passkeys just now. Reload the page to try again.
      </p>
    );
  }
  const { passkeys } = listed.data;
  if (passkeys.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">
        A passkey signs you in with your fingerprint, face or screen lock instead of a password.
      </p>
    );
  }
  return (
    <>
      <ul className="flex flex-col divide-y">
        {passkeys.map((passkey) => (
          <PasskeyRow
            key={passkey.id}
            passkey={passkey}
            removing={remove.isPending}
            onRemove={() => {
              remove.mutate(passkey.id);
            }}
          />
        ))}
      </ul>
      <PasskeyNotice error={remove.error} />
    </>
  );
}
