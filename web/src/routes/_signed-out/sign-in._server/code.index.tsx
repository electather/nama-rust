import { createFileRoute } from "@tanstack/react-router";

import { CodeSignIn } from "@/features/auth/components/code-sign-in";

export const Route = createFileRoute("/_signed-out/sign-in/_server/code/")({
  component: CodeSignInRoute,
});

function CodeSignInRoute() {
  const { redirect } = Route.useSearch();
  return <CodeSignIn redirect={redirect} />;
}
