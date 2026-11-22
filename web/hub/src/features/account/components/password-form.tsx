import { PasswordFormNote } from "@hub/features/account/components/password-form-note";
import { TextField } from "@hub/features/account/components/text-field";
import { usePasswordStatus, useSetPassword } from "@hub/features/account/data/password";
import { credentialErrors } from "@hub/features/account/lib/refusals";
import { Button } from "nama-ui/components/button";

/** What the form says to an Account without a password, and to one with. */
const wording = {
  set: {
    label: "Password",
    hint: "8 to 1024 characters. You can sign in with it as well as with Sign-in Links.",
    action: "Set password",
  },
  change: {
    label: "New password",
    hint: "8 to 1024 characters. Changing it signs you out everywhere else.",
    action: "Change password",
  },
};

/** Sets a password on the signed-in Account, or changes the one it has. */
export function PasswordForm() {
  const status = usePasswordStatus();
  const setPassword = useSetPassword();
  if (!status) {
    return null;
  }
  const words = status.hasPassword ? wording.change : wording.set;
  const errors = setPassword.error ? credentialErrors(setPassword.error) : {};
  return (
    <form
      className="flex flex-col gap-5"
      onSubmit={(event) => {
        event.preventDefault();
        const form = event.currentTarget;
        const password = new FormData(form).get("password");
        setPassword.mutate(
          { password: typeof password === "string" ? password : "" },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      }}
    >
      {/* Tells a password manager whose password this is. */}
      <input
        type="email"
        name="email"
        autoComplete="username"
        value={status.email}
        readOnly
        hidden
      />
      <TextField
        label={words.label}
        name="password"
        type="password"
        autoComplete="new-password"
        hint={words.hint}
        error={errors.password}
      />
      <PasswordFormNote error={setPassword.error} saved={setPassword.isSuccess} />
      <Button type="submit" size="lg" disabled={setPassword.isPending}>
        {submitLabel(words.action, setPassword.isPending)}
      </Button>
    </form>
  );
}

function submitLabel(action: string, pending: boolean) {
  return pending ? "Saving…" : action;
}
