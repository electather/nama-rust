import { PasskeyNotice } from "@hub/features/account/components/passkey-notice";
import { TextField } from "@hub/features/account/components/text-field";
import { useAccount } from "@hub/features/account/data/account";
import { useAddPasskey } from "@hub/features/account/data/passkeys";
import { Button } from "nama-ui/components/button";

/** Names a new passkey, then has the browser create it and adds it to the Account. */
export function AddPasskeyForm() {
  const account = useAccount();
  const add = useAddPasskey();
  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        const form = event.currentTarget;
        const name = new FormData(form).get("name");
        add.mutate(
          { email: account?.email ?? "", name: typeof name === "string" ? name : "" },
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
        error={undefined}
      />
      <Button type="submit" variant="outline" disabled={add.isPending}>
        {add.isPending ? "Adding passkey…" : "Add passkey"}
      </Button>
      <PasskeyNotice error={add.error} />
    </form>
  );
}
