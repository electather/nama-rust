import { Code } from "@connectrpc/connect";
import { useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";

import { FormField } from "@/components/form-field";
import { SignInLayout } from "@/components/sign-in-layout";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SessionDelivery, SignInService } from "@/gen/nama/v1/sign_in_pb";
import { isSetupRequired } from "@/lib/access";
import { type FormErrors, formText, refusal } from "@/lib/forms";

export const Route = createFileRoute("/sign-in")({
  // The router merges this over the raw search, so a refused `redirect` has to be overwritten.
  validateSearch: (search): { redirect?: string | undefined } => ({
    redirect: pathOnThisServer(search["redirect"]),
  }),
  beforeLoad: async ({ context }) => {
    if (await isSetupRequired(context)) throw redirect({ to: "/setup" });
  },
  component: SignIn,
});

function SignIn() {
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  const queryClient = useQueryClient();
  const server = useQuery(ServerService.method.getServerInfo);
  const signIn = useMutation(SignInService.method.signIn, {
    onSuccess: async () => {
      // Forget the signed-out visitor, and anything cached for whoever was signed in before.
      queryClient.clear();
      await navigate({ href: search.redirect ?? "/" });
    },
  });
  const errors = signIn.error ? signInErrors(signIn.error) : {};

  return (
    <SignInLayout
      heading={server.data ? `Sign in to ${server.data.name}` : "Sign in"}
      intro="Sign in with the username and password you were given for this Server."
      action={signIn.isPending ? "Signing in…" : "Sign in"}
      pending={signIn.isPending}
      error={errors.form}
      onSubmit={(form) => {
        signIn.mutate({
          name: formText(form, "name"),
          password: formText(form, "password"),
          delivery: SessionDelivery.COOKIE,
        });
      }}
    >
      <FormField label="Username" name="name" autoComplete="username" maxLength={64} />
      <FormField
        label="Password"
        name="password"
        type="password"
        autoComplete="current-password"
        error={errors.password}
      />
    </SignInLayout>
  );
}

function signInErrors(error: unknown): FormErrors {
  return refusal(error, "sign in", {
    // The Server doesn’t say whether it was the username or the password, so neither do we.
    [Code.Unauthenticated]: { password: "That username and password don’t match." },
  });
}

// Resolving `value` against a stand-in origin keeps only paths on this Server, so a crafted
// `?redirect=` can't send someone elsewhere after they sign in.
const standIn = "http://nama.invalid";
function pathOnThisServer(value: unknown) {
  const url = typeof value === "string" ? URL.parse(value, standIn) : null;
  return url?.origin === standIn ? `${url.pathname}${url.search}${url.hash}` : undefined;
}
