import { SetupFields } from "@/features/auth/components/setup-fields";
import type { useSetup } from "@/features/auth/data/use-setup";
import { useSetupHeading } from "@/features/auth/hooks/use-setup-heading";
import { setupErrors } from "@/features/auth/lib/sign-in-errors";
import { SessionDelivery } from "@/gen/nama/v1/sign_in_pb";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { formText } from "@/shared/lib/forms";

/** The setup form: the Setup Code, then the first Admin's name and password. */
export function SetupForm({ setup }: { setup: ReturnType<typeof useSetup> }) {
  const errors = setup.error ? setupErrors(setup.error) : {};
  return (
    <SignInLayout
      heading={useSetupHeading()}
      action={setup.isPending ? "Setting up…" : "Set up"}
      pending={setup.isPending}
      errors={errors}
      onSubmit={(form) => {
        setup.mutate({
          setupCode: formText(form, "setupCode"),
          name: formText(form, "name"),
          password: formText(form, "password"),
          delivery: SessionDelivery.COOKIE,
        });
      }}
    >
      <SetupFields errors={errors} />
    </SignInLayout>
  );
}
