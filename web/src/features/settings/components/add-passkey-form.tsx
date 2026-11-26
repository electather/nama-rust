import { Button } from "nama-ui/components/button";
import { Input } from "nama-ui/components/input";
import { useState } from "react";

import { passkeyError } from "@/features/auth/lib/hub-errors";
import { HubRefusal } from "@/features/settings/components/hub-refusal";
import { useAddPasskey } from "@/features/settings/data/use-add-passkey";
import { useHubAccount } from "@/features/settings/data/use-hub-account";
import { useOffersPasskeys } from "@/features/settings/data/use-offers-passkeys";
import { AlertText } from "@/shared/components/form-field/alert-text";
import { SettingRow } from "@/shared/components/setting-row";
import { formText, onSubmitted } from "@/shared/lib/forms";

/**
 * Names a new passkey, then has the browser create it and adds it to the Account, where the Hub
 * offers passkeys and the browser has them.
 */
export function AddPasskeyForm() {
  const { email } = useHubAccount();
  const add = useAddPasskey();
  // Caught here, since the Hub refuses a blank name only after the browser created the passkey.
  const [blank, setBlank] = useState(false);
  if (!useOffersPasskeys()) {
    return null;
  }
  return (
    <form
      noValidate
      className="flex flex-col"
      onSubmit={onSubmitted((typed, form) => {
        const name = formText(typed, "name").trim();
        setBlank(name === "");
        if (name !== "") {
          add.mutate(
            { email, name },
            {
              onSuccess: () => {
                form.reset();
              },
            },
          );
        }
      })}
    >
      <SettingRow
        wide
        label="New passkey’s name"
        hint="So you can tell your passkeys apart."
        error={blank ? "Give it a name, such as the device it’s on." : undefined}
        size="field"
      >
        {(control) => (
          <Input {...control} name="name" maxLength={64} placeholder="Phone" aria-invalid={blank} />
        )}
      </SettingRow>
      <div className="flex justify-end pt-2.5">
        <Button type="submit" variant="secondary" disabled={add.isPending}>
          {add.isPending ? "Adding passkey…" : "Add passkey"}
        </Button>
      </div>
      <AlertText
        error={
          <HubRefusal
            error={add.error}
            said={passkeyError(add.error)}
            toDo="to change your passkeys"
          />
        }
      />
    </form>
  );
}
