import { Link } from "@tanstack/react-router";

import { useFinishSignIn } from "@/features/auth/data/use-finish-sign-in";
import { useSwitchUser } from "@/features/auth/data/use-switch-user";
import { switchErrors } from "@/features/auth/lib/switch-errors";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { FormField } from "@/shared/components/form-field/form-field";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { formText } from "@/shared/lib/forms";
import { textLink } from "@/shared/lib/text-link-classes";

/** Asking for `user`'s Switch PIN before switching this Device to them, then going on to `redirect`. */
export function SwitchPinPrompt({ user, redirect }: { user: User; redirect: string | undefined }) {
  const switchUser = useSwitchUser(useFinishSignIn(redirect));
  const errors = switchUser.error ? switchErrors(switchUser.error, user.name) : {};
  return (
    <SignInLayout
      heading={`Enter ${user.name}’s Switch PIN`}
      action={switchUser.isPending ? "Switching…" : `Switch to ${user.name}`}
      pending={switchUser.isPending}
      errors={errors}
      onSubmit={(form) => {
        switchUser.mutate({ userId: user.id, switchPin: formText(form, "switchPin") });
      }}
      footer={
        <p className="text-center text-sm text-muted-foreground">
          <Link to="/switch-user" search={{ redirect }} className={textLink}>
            Pick another User
          </Link>
        </p>
      }
    >
      <FormField
        label="Switch PIN"
        name="switchPin"
        variant="lg"
        type="password"
        inputMode="numeric"
        autoComplete="off"
        maxLength={6}
        error={errors.switchPin}
      />
    </SignInLayout>
  );
}
