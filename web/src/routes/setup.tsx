import { Code } from "@connectrpc/connect";
import { useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";

import { DeviceNameField, FormField } from "@/components/form-field";
import { SignInLayout } from "@/components/sign-in-layout";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SessionDelivery, SignInService } from "@/gen/nama/v1/sign_in_pb";
import { isSetupRequired } from "@/lib/access";
import { type FormErrors, formText, refusal, serverMessage } from "@/lib/forms";

export const Route = createFileRoute("/setup")({
  beforeLoad: async ({ context }) => {
    if (!(await isSetupRequired(context))) throw redirect({ to: "/" });
  },
  component: Setup,
});

function Setup() {
  const navigate = Route.useNavigate();
  const queryClient = useQueryClient();
  const server = useQuery(ServerService.method.getServerInfo);
  const setup = useMutation(SignInService.method.setup, {
    onSuccess: async () => {
      // The cached Server info still says setup is needed, and nobody is signed in.
      queryClient.clear();
      await navigate({ to: "/" });
    },
  });
  const errors = setup.error ? setupErrors(setup.error) : {};

  return (
    <SignInLayout
      heading={server.data ? `Set up ${server.data.name}` : "Set up this Server"}
      intro="Enter the Setup Code this Server printed to its log when it started, then create the first Admin: you."
      action={setup.isPending ? "Setting up…" : "Set up"}
      pending={setup.isPending}
      error={errors.form}
      onSubmit={(form) => {
        setup.mutate({
          setupCode: formText(form, "setupCode"),
          name: formText(form, "name"),
          password: formText(form, "password"),
          deviceName: formText(form, "deviceName"),
          delivery: SessionDelivery.COOKIE,
        });
      }}
    >
      <SetupFields errors={errors} />
    </SignInLayout>
  );
}

function SetupFields({ errors }: { errors: FormErrors }) {
  return (
    <>
      <FormField
        label="Setup Code"
        name="setupCode"
        autoComplete="off"
        spellCheck={false}
        error={errors.setupCode}
      />
      <FormField
        label="Your name"
        name="name"
        autoComplete="username"
        maxLength={64}
        error={errors.name}
      />
      <FormField
        label="Password"
        name="password"
        type="password"
        autoComplete="new-password"
        error={errors.password}
      />
      <DeviceNameField error={errors.deviceName} />
    </>
  );
}

function setupErrors(error: unknown): FormErrors {
  return refusal(error, "set up this Server", {
    [Code.PermissionDenied]: {
      setupCode: "That isn’t the Setup Code. Check the Server’s log for it.",
    },
    [Code.FailedPrecondition]: {
      form: "This Server is already set up. Reload the page to sign in.",
    },
    [Code.InvalidArgument]: { [invalidField(error)]: serverMessage(error) },
  });
}

// ponytail: the Server names the field only in its message ("a Device's name needs…"). Read it
// from Connect error details instead if the Server starts sending them.
function invalidField(error: unknown): Exclude<keyof FormErrors, "form"> {
  const message = String(error);
  if (message.includes("Device")) return "deviceName";
  if (message.includes("User's name")) return "name";
  return "password";
}
