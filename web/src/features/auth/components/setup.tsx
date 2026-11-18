import { SetupFields } from "@/features/auth/components/setup-fields";
import { useSetup } from "@/features/auth/data/use-setup";
import { setupErrors } from "@/features/auth/lib/sign-in-errors";
import { SessionDelivery } from "@/gen/nama/v1/sign_in_pb";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { useServerName } from "@/shared/hooks/use-server-name";
import { formText } from "@/shared/lib/forms";

/** The page that creates the first Admin, with the Setup Code the Server printed to its log. */
export function Setup() {
  const serverName = useServerName();
  const setup = useSetup();
  const errors = setup.error ? setupErrors(setup.error) : {};

  return (
    <SignInLayout
      heading={serverName === undefined ? "Set up this Server" : `Set up ${serverName}`}
      intro="Enter the Setup Code this Server printed to its log when it started, then create the first Admin: you."
      action={setup.isPending ? "Setting up…" : "Set up"}
      pending={setup.isPending}
      error={errors.form}
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
