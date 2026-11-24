import { PasskeyNotice } from "@hub/features/account/components/passkey-notice";
import { TextField } from "@hub/features/account/components/text-field";
import { useAccount } from "@hub/features/account/data/account";
import { useAddPasskey, usePasskeysWork } from "@hub/features/account/data/passkeys";
import { Button } from "nama-ui/components/button";
import { useState } from "react";

/**
 * Names a new passkey, then has the browser create it and adds it to the Account, where passkeys
 * work.
 */
export function AddPasskeyForm() {
  const account = useAccount();
  const add = useAddPasskey();
  // Caught here, since the Hub refuses a blank name only after the browser created the passkey.
  const [blank, setBlank] = useState(false);
  if (!usePasskeysWork()) {
    return null;
  }
  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        const form = event.currentTarget;
        const name = nameIn(form);
        setBlank(name === undefined);
        if (name === undefined) {
          return;
        }
        add.mutate(
          { email: account?.email ?? "", name },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      }}
    >
      <TextField
        label="New passkey’s name"
        name="name"
        maxLength={64}
        placeholder="Phone"
        hint="So you can tell your passkeys apart."
        error={blank ? "Give it a name, such as the device it’s on." : undefined}
      />
      <Button type="submit" variant="outline" disabled={add.isPending}>
        {add.isPending ? "Adding passkey…" : "Add passkey"}
      </Button>
      <PasskeyNotice error={add.error} />
    </form>
  );
}

/** The name typed in `form`, unless it is blank. */
function nameIn(form: HTMLFormElement) {
  const name = new FormData(form).get("name");
  return typeof name === "string" && name.trim() !== "" ? name : undefined;
}
