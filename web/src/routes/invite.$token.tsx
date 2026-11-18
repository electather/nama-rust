import { Code } from "@connectrpc/connect";
import { useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { SessionDelivery, SignInService } from "@/gen/nama/v1/sign_in_pb";
import { FormField } from "@/shared/components/form-field/form-field";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { type FormErrors, formText, refusedFieldErrors, refusal } from "@/shared/lib/forms";

export const Route = createFileRoute("/invite/$token")({
  component: AcceptInvite,
});

function AcceptInvite() {
  const { token } = Route.useParams();
  const navigate = Route.useNavigate();
  const queryClient = useQueryClient();
  const server = useQuery(ServerService.method.getServerInfo);
  const accept = useMutation(SignInService.method.acceptInvite, {
    onSuccess: async () => {
      // Forget anything cached for whoever was signed in on this browser before.
      queryClient.clear();
      await navigate({ to: "/" });
    },
  });
  const errors = accept.error ? acceptErrors(accept.error) : {};

  return (
    <SignInLayout
      heading={server.data ? `Join ${server.data.name}` : "Join this Server"}
      intro="You’ve been invited. Pick the name and password you’ll sign in with."
      action={accept.isPending ? "Joining…" : "Join"}
      pending={accept.isPending}
      error={errors.form}
      onSubmit={(form) => {
        accept.mutate({
          token,
          name: formText(form, "name"),
          password: formText(form, "password"),
          delivery: SessionDelivery.COOKIE,
        });
      }}
    >
      <InviteFields errors={errors} />
    </SignInLayout>
  );
}

function InviteFields({ errors }: { errors: FormErrors }) {
  return (
    <>
      <FormField
        label="Username"
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
    </>
  );
}

function acceptErrors(error: unknown): FormErrors {
  return refusal(error, "join this Server", {
    [Code.NotFound]: {
      form: "This Invite no longer works: it was used, revoked or has expired. Ask whoever sent it for a new one.",
    },
    [Code.AlreadyExists]: { name: "Someone already has that name. Pick another." },
    [Code.InvalidArgument]: refusedFieldErrors(error, ["name", "password"]),
  });
}
