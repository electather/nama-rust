import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { useState } from "react";

import { CodeSignIn } from "@/components/code-sign-in";
import { PasswordSignIn } from "@/components/password-sign-in";
import { Button } from "@/components/ui/button";
import { isSetupRequired } from "@/lib/access";

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
  const [withCode, setWithCode] = useState(false);
  async function signedIn() {
    // Forget the signed-out visitor, and anything cached for whoever was signed in before.
    queryClient.clear();
    await navigate({ href: search.redirect ?? "/" });
  }

  const otherWay = (label: string) => (
    <Button
      variant="ghost"
      onClick={() => {
        setWithCode(!withCode);
      }}
    >
      {label}
    </Button>
  );
  return withCode ? (
    <CodeSignIn onSignedIn={signedIn} footer={otherWay("Sign in with a password")} />
  ) : (
    <PasswordSignIn onSignedIn={signedIn} footer={otherWay("Sign in with a code")} />
  );
}

// Resolving `value` against a stand-in origin keeps only paths on this Server, so a crafted
// `?redirect=` can't send someone elsewhere after they sign in.
const standIn = "http://nama.invalid";
function pathOnThisServer(value: unknown) {
  const url = typeof value === "string" ? URL.parse(value, standIn) : null;
  return url?.origin === standIn ? `${url.pathname}${url.search}${url.hash}` : undefined;
}
